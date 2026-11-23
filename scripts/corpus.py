#!/usr/bin/env python3
"""Tracked crate regression suite. Python stdlib only; Cargo is a CI oracle only."""
import argparse
from collections import Counter
import datetime
import difflib
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import tomllib
import urllib.request

from corpus_transport import CONTACT, IndexCache, MetadataFixture, NoRedirect, shard
from check_crates_phase0 import cargo_environment, project, lock_ids
from crates_phase0_fixtures import cases as synthetic_cases, dep, record

REPO = Path(__file__).resolve().parents[1]
CORPUS = REPO / "tests/corpus"
IDENTITY = ("name", "vers", "cksum", "deps", "features", "features2", "links", "v")



def digest(data):
    return hashlib.sha256(data).hexdigest()


def dump(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def load(path):
    return json.loads(Path(path).read_text())


def bundle(path):
    with gzip.open(path, "rb") as stream:
        raw = stream.read(512 * 1024 * 1024 + 1)
    if len(raw) > 512 * 1024 * 1024:
        raise ValueError("frozen metadata bundle exceeds budget")
    obj = json.loads(raw)
    if obj["schema"] != 1:
        raise ValueError("unsupported index bundle")
    return {p: data.encode() for p, data in obj["files"].items()}


def save_bundle(path, files):
    data = {"schema": 1, "created_at": datetime.date.today().isoformat(),
            "source": {"url":"https://index.crates.io/", "policy":"one coherent conditional-request snapshot per run"},
            "files": {p: b.decode() for p, b in sorted(files.items())}}
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_bytes(gzip.compress(json.dumps(data, sort_keys=True, separators=(",", ":")).encode(), mtime=0))


def corpus(path):
    obj = load(path)
    if obj.get("schema") != 1 or not obj.get("created_at") or not obj.get("source"):
        raise ValueError("corpus needs schema 1, creation date and source")
    return obj


def semantic_rows(data):
    result = {}
    for line in data.splitlines():
        row = json.loads(line)
        identity = (row["name"], row["vers"])
        if identity in result and result[identity] != row:
            raise ValueError("duplicate exact index identity")
        result[identity] = row
    return result


def upstream_delta(old, new):
    """Only new versions, yank changes and nonidentity field drift are soft changes.

    Mutation/removal of an existing checksum/dependency/feature identity never earns
    an upstream exemption. Byte-only formatting changes are explicit metadata drift.
    """
    details = []
    for path in sorted(set(old) | set(new)):
        if path not in old:
            details.append(dict(path=path, kind="new_index"))
            continue
        if path not in new:
            return False, [dict(path=path, kind="missing_index")]
        if old[path] == new[path]:
            continue
        before, after = semantic_rows(old[path]), semantic_rows(new[path])
        for identity, row in before.items():
            if identity not in after:
                return False, [dict(path=path, identity=list(identity), kind="removed_identity")]
            if any(row.get(k) != after[identity].get(k) for k in IDENTITY):
                return False, [dict(path=path, identity=list(identity), kind="identity_mutation")]
        added = sorted(set(after) - set(before))
        yanks = sorted(i for i in before if before[i].get("yanked", False) != after[i].get("yanked", False))
        backfills = sorted(i for i in before if before[i] != after[i] and i not in yanks)
        details.append(dict(path=path, kind="upstream_metadata", added=[list(i) for i in added],
                            yanks=[list(i) for i in yanks], backfills=[list(i) for i in backfills],
                            formatting_only=not added and not yanks and not backfills))
    return True, details


def classify(expected, observed, replay, old_inputs, new_inputs, oracle_ok=True):
    if not oracle_ok:
        return "conformance_mismatch", ["Cargo/native coverage, refusal or filtered-index check differs"]
    if expected is None:
        return "regression", ["missing reviewed expected result"]
    if replay != expected:
        return "regression", ["current binary differs on the exact frozen baseline inputs"]
    if observed == expected and old_inputs == new_inputs:
        return "clean", []
    permitted, details = upstream_delta(old_inputs, new_inputs)
    if not permitted:
        return "regression", details
    if not details:
        return "regression", ["output changed without an upstream input change"]
    # Newly refused resolution is not excused by an unrelated version/backfill.
    # Only a demonstrably yanked declared root receives the documented refusal exemption.
    if expected["status"] == "ok" and observed["status"] != "ok":
        roots = {(p["name"], p["version"]) for p in expected.get("profiles", [])}
        yanked = {tuple(i) for d in details for i in d.get("yanks", [])}
        if not roots & yanked:
            return "regression", ["new refusal is not explained by a declared-root yank", *details]
    return "upstream", details


def invoke(binary, args, work, label, network=()):
    started = time.monotonic()
    result = subprocess.run([str(binary), "--events", "jsonl", "crates", *map(str, args), *network],
                            text=True, capture_output=True, timeout=600)
    (work / (label + ".jsonl")).write_text(result.stdout)
    (work / (label + ".stderr")).write_text(result.stderr)
    events = [json.loads(line) for line in result.stdout.splitlines()]
    if not events or events[0].get("type") != "hello" or events[0].get("protocol") != 1:
        raise ValueError("missing CLI protocol hello")
    if events[-1].get("type") != "result" or sum(e.get("type") == "result" for e in events) != 1:
        raise ValueError("missing or duplicate CLI result")
    if bool(result.returncode == 0) != events[-1]["ok"]:
        raise ValueError("CLI exit status disagrees with typed result")
    if any(e.get("type") == "artifact" and e.get("path", "").endswith(".crate") for e in events):
        raise ValueError("planning attempted an archive operation")
    return events[-1], time.monotonic() - started


def catalog(roots):
    text = "[crates]\n"
    for n, root in enumerate(roots):
        value = {"package": root["name"], "version": "=" + root["version"],
                 "features": root["features"], "default-features": root["default_features"]}
        text += f'"r{n:04d}" = {{' + ", ".join(k + "=" + json.dumps(v) for k, v in value.items()) + "}\n"
    return text + ('\n[crate-policy]\nhosts=["x86_64-unknown-linux-gnu","x86_64-pc-windows-msvc"]\n'
                   'targets=["x86_64-unknown-linux-gnu","x86_64-pc-windows-msvc"]\n'
                   '[crate-output]\nbase-url="https://mirror.example/corpus"\n'
                   '[crate-network]\ncontact=' + json.dumps(CONTACT) + '\nconcurrency=1\nattempts=1\n')


def protocol_value(value):
    # The events adapter passes through TOML, where optional None table fields are
    # omitted. Compare that documented wire representation, preserving all values.
    if isinstance(value, dict):
        return {k: protocol_value(v) for k, v in value.items() if v is not None}
    if isinstance(value, list):
        return [protocol_value(v) for v in value]
    return value


def summarize(plan, result, profiles):
    if not result["ok"]:
        return dict(status="refused", profiles=profiles, refusal_kind=result["error"]["kind"],
                    version_union=[], roots=[], selected_rows={}, index_warnings=[], metadata_warnings=[])
    versions = plan["versions"]
    ids = sorted(v["name"] + "@" + v["version"] for v in versions)
    rows = {}
    for v in versions:
        rows[v["name"] + "@" + v["version"]] = json.loads(v["index_line"])
    roots = []
    if plan["roots"]:
        for n, r in enumerate(plan["roots"]):
            coverage = sorted(v["name"] + "@" + v["version"] for v in versions
                              if any(reason["root"] == n for reason in v["reasons"]))
            roots.append(dict(profile=r["profile"], resolved_versions=coverage))
    else:
        roots = [dict(profile=p, resolved_versions=ids) for p in profiles]
    # Timestamps, plan_id, wall time and raw metadata hashes are recorded elsewhere;
    # they are never mistaken for deterministic resolver-output regressions.
    return dict(status="ok", profiles=profiles, roots=roots, version_union=ids, selected_rows=rows,
                known_archive_bytes=sum(v["expected_bytes"] or 0 for v in versions),
                unknown_archives=sum(v["expected_bytes"] is None for v in versions),
                version_details={v["name"]+"@"+v["version"]:{k:value for k,value in v.items() if k != "index_line"} for v in versions},
                index_line_sha256={v["name"]+"@"+v["version"]:digest(v["index_line"].encode()) for v in versions},
                warnings=plan["warnings"], index_warnings=plan.get("index_warnings", []),
                metadata_warnings=result.get("report", {}).get("metadata_warnings", []))


def native_case(binary, case, work, fixture):
    work.mkdir(parents=True)
    config = work / "catalog.toml"
    config.write_text(catalog(case["roots"]))
    if case.get("lock"):
        entry = case["lock"]
        lock = CORPUS / entry["lock_file"]
        if digest(lock.read_bytes()) != entry["lock_sha256"]:
            raise ValueError("pinned project lock checksum changed")
        result, _ = invoke(binary, ["import-lock", "--config", config, "--lock", entry["name"] + "=" + str(lock), "--write"], work, "import")
        if not result["ok"]:
            raise ValueError("pinned project lock import refused")
    fixture.observed = {}
    plan_path = work / "plan.json"
    result, elapsed = invoke(binary, ["plan", "--config", config, "--output", plan_path], work, "plan", fixture.network)
    plan = load(plan_path) if result["ok"] else None
    profiles = case["roots"]
    if case.get("lock"):
        profiles = [dict(name=case["id"], version=case["lock"]["commit"], features=[], default_features=False)]
    semantic = summarize(plan, result, profiles)
    if plan:
        shown, _ = invoke(binary, ["show", "--plan", plan_path], work, "show")
        if not shown["ok"] or protocol_value(shown["report"]) != protocol_value(plan):
            raise ValueError("frozen output did not replay byte-identical plan data")
    return semantic, plan, dict(fixture.observed), elapsed, result.get("error")


def oracle(case, plan, work, fixture, binary):
    """Real Cargo lock resolution; no compile, fetch or archive API is available."""
    fixture.filtered = {}
    if plan:
        for v in plan["versions"]:
            p = shard(v["name"])
            fixture.filtered[p] = fixture.filtered.get(p, b"") + v["index_line"].encode()
    if case.get("lock"):
        return lock_oracle(case, plan, work, fixture)
    roots = [r["profile"] for r in plan["roots"]] if plan else case["roots"]
    reports = []
    all_ok = True
    for n, root in enumerate(roots):
        choices = {}
        timings = {}
        per_root_ok = True
        if plan and len(case["roots"]) > 1:
            independent, _, _, seconds, _ = native_case(binary, dict(case, roots=[root]), work / f"native-root-{n}", fixture)
            timings["native_seconds"] = seconds
            original_coverage = sorted(v["name"]+"@"+v["version"] for v in plan["versions"] if any(r["root"] == n for r in v["reasons"]))
            per_root_ok = independent["status"] == "ok" and independent["version_union"] == original_coverage
        for mode in ("full", "filtered") if plan else ("full",):
            p = work / f"cargo-{n}-{mode}"
            project(p, root, fixture.base + "/cargo", mode)
            started = time.monotonic()
            result = subprocess.run(["cargo", "generate-lockfile", "--manifest-path", str(p / "Cargo.toml")],
                                    cwd=p, env=cargo_environment(work / f"home-{n}-{mode}"),
                                    text=True, capture_output=True, timeout=180)
            timings[mode+"_seconds"] = time.monotonic()-started
            (work / f"cargo-{n}-{mode}.log").write_text(result.stdout + result.stderr)
            choices[mode] = lock_ids(p) if result.returncode == 0 else None
        coverage = {v["name"] + "@" + v["version"] for v in plan["versions"]
                    if any(r["root"] == n for r in v["reasons"])} if plan else set()
        if plan:
            ok = (per_root_ok and choices["full"] is not None and choices["filtered"] is not None
                  and choices["full"] <= coverage and choices["filtered"] <= coverage)
        else:
            ok = choices["full"] is None
        all_ok &= ok
        cargo_ids = choices["full"] or set()
        reports.append(dict(profile=root, ok=ok, timings=timings, cargo_versions=sorted(cargo_ids),
                            filtered_versions=sorted(choices.get("filtered") or []),
                            witness_versions=plan["witnesses"][n]["versions"] if plan else [],
                            native_only=sorted(coverage - cargo_ids), cargo_only=sorted(cargo_ids - coverage),
                            known_archive_bytes=sum(v["expected_bytes"] or 0 for v in plan["versions"] if v["name"]+"@"+v["version"] in coverage) if plan else 0,
                            unknown_archives=sum(v["expected_bytes"] is None for v in plan["versions"] if v["name"]+"@"+v["version"] in coverage) if plan else 0,
                            version_overfetch_percent=100 * len(coverage - cargo_ids) / len(cargo_ids) if cargo_ids else None,
                            archive_byte_overfetch=None))
    return all_ok, reports


def lock_oracle(case, plan, work, fixture):
    """Cargo checks the registry-only projection, preserving original locked yanks.

    Exact aliases keep every imported registry pin as a root. update --workspace
    retains lock preferences instead of treating yanked pins as new unlocked roots.
    Git/path/workspace packages and project compilation are outside this projection.
    """
    expected = {v["name"] + "@" + v["version"]: v["cksum"] for v in plan["versions"]}
    choices = {}
    timings = {}
    for mode in ("full", "filtered"):
        path = work / ("cargo-lock-" + mode)
        project(path, dict(name="itoa", version="1.0.15"), fixture.base + "/cargo", mode)
        text = '[package]\nname="corpus-lock-projection"\nversion="0.0.0"\nedition="2021"\nresolver="2"\n[dependencies]\n'
        for n, v in enumerate(plan["versions"]):
            text += f'p{n:04d}={{package={json.dumps(v["name"])},version={json.dumps("="+v["version"])},default-features=false}}\n'
        (path / "Cargo.toml").write_text(text)
        (path / "Cargo.lock").write_bytes((CORPUS / case["lock"]["lock_file"]).read_bytes())
        started = time.monotonic()
        result = subprocess.run(["cargo", "update", "--workspace", "--manifest-path", str(path / "Cargo.toml")],
                                cwd=path, env=cargo_environment(work / ("home-lock-"+mode)),
                                text=True,capture_output=True,timeout=300)
        timings[mode+"_seconds"]=time.monotonic()-started
        (work/("cargo-lock-"+mode+".log")).write_text(result.stdout+result.stderr)
        choices[mode] = None
        if result.returncode == 0:
            lock = tomllib.loads((path / "Cargo.lock").read_text())
            choices[mode] = {p["name"]+"@"+p["version"]:p.get("checksum") for p in lock["package"] if "source" in p}
    ok = all(value == expected for value in choices.values())
    return ok, [dict(profile=dict(name=case["id"],version=case["lock"]["commit"]),ok=ok,
                     cargo_versions=sorted(choices["full"] or {}),filtered_versions=sorted(choices["filtered"] or {}),
                     native_only=sorted(set(expected)-set(choices["full"] or {})),
                     cargo_only=sorted(set(choices["full"] or {})-set(expected)),
                     version_overfetch_percent=0 if ok else None, archive_byte_overfetch=None, timings=timings,
                     scope="registry pin/checksum projection with original lock preferences, not a project build")]


def definitions(fast):
    top = corpus(CORPUS / "top-crates.json")["roots"]
    frozen = corpus(CORPUS / "frozen-top100.json")
    if digest((REPO / frozen["source"]["fixture"]).read_bytes()) != frozen["source"]["fixture_sha256"]:
        raise ValueError("frozen 2026-11-13 roots were modified")
    result = [dict(id="frozen-top100", corpus="frozen-top100", roots=frozen["roots"], frozen=True),
              dict(id="top-crates", corpus="top-crates", roots=top)]
    tricky = corpus(CORPUS / "tricky.json")
    synthetic = {name: (entries, profile, success) for name, entries, profile, success in synthetic_cases()}
    for case in tricky["cases"]:
        c = dict(case, corpus="tricky")
        if c.get("synthetic"):
            entries, profile, success = synthetic[c["synthetic"]]
            c["roots"] = [dict(name="root", version="1.0.0", features=[], default_features=True) | profile]
            c["expected_status"] = "ok" if success else "refused"
            files = {}
            for r in entries:
                p = shard(r["name"])
                files[p] = files.get(p, b"") + (json.dumps(r) + "\n").encode()
            c["input_override"] = files
        result.append(c)
    for entry in corpus(CORPUS / "locks.json")["projects"]:
        result.append(dict(id="lock-" + re.sub(r"[^a-z0-9-]", "-", entry["name"]), corpus="locks", roots=[], lock=entry, fast=entry["name"] == "ripgrep-12.1.1"))
    if fast:
        # Whole top-100 plans are cheap and retain the frozen 100/100 union guard;
        # only Cargo per-root oracles are sampled for PR speed.
        result = [c for c in result if c["corpus"] in ("top-crates", "frozen-top100") or c.get("fast", False)]
    for c in result:
        if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,90}", c["id"]):
            raise ValueError("unsafe corpus case identifier")
    return result


def run_suite(args):
    if args.oracle and os.environ.get("GITHUB_ACTIONS") != "true":
        raise ValueError("real Cargo is a CI oracle only; omit --oracle for local planning")
    work = args.output.resolve()
    work.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    version = tomllib.loads((REPO / "Cargo.toml").read_text())["package"]["version"]
    cache = IndexCache(args.cache or work / "cache", f"rust-mirror/{version} (+https://github.com/rustcommons/rust-mirror; {CONTACT})")
    historical_path = CORPUS / "inputs/frozen-top100-20261113.json.gz"
    frozen_source = corpus(CORPUS / "frozen-top100.json")["source"]
    if digest(historical_path.read_bytes()) != frozen_source["index_bundle_sha256"]:
        raise ValueError("frozen historical sparse bundle changed")
    historical = bundle(historical_path)
    baseline_inputs_path = CORPUS / "inputs/baseline.json.gz"
    baseline_inputs = bundle(baseline_inputs_path) if baseline_inputs_path.exists() else {}
    rows = []
    observed_live = {}
    with MetadataFixture(cache).serve() as fixture:
        for case in definitions(args.fast):
            print("Corpus:", case["id"], flush=True)
            expected_file = CORPUS / "expected" / (case["id"] + ".json")
            expected_document = load(expected_file) if expected_file.exists() else None
            expected = expected_document["expected"] if expected_document else None
            override = historical if case.get("frozen") else case.get("input_override")
            try:
                fixture.override = override if override is not None else (baseline_inputs if expected else None)
                replay, _, replay_inputs, _, _ = native_case(binary, case, work / case["id"] / "replay", fixture)
                fixture.override = override
                observed, plan, inputs, elapsed, error = native_case(binary, case, work / case["id"] / "live", fixture)
                # A changed graph may stop consulting an old dependency index.
                # Revalidate those observations too: an unused path is not an
                # upstream deletion and must not become a false regression.
                current_inputs = {p: fixture.full(p) for p in set(inputs) | set(replay_inputs)}
                prior_inputs = {p: (override if override is not None else baseline_inputs)[p] for p in replay_inputs} if expected else current_inputs
                ok, cargo = True, []
                if args.oracle:
                    oracle_case = case
                    if args.fast and plan and len(plan["roots"]) > 8:
                        # Run only eight representative roots while keeping the full
                        # native 100-root plan and version union in the report.
                        plan = dict(plan, roots=plan["roots"][:8])
                    ok, cargo = oracle(oracle_case, plan, work / case["id"] / "live", fixture, binary)
                if fixture.archive_requests or fixture.errors:
                    raise ValueError("metadata-only transport violation: " + repr(fixture.archive_requests + fixture.errors))
                if args.record:
                    expected = observed
                    dump(work / "proposed-baseline" / "expected" / expected_file.name, dict(schema=1, created_at=datetime.date.today().isoformat(), source={"product_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO,text=True).strip(), "case":case["id"]}, expected=observed))
                    label, difference = "recorded", []
                else:
                    label, difference = classify(expected, observed, replay, prior_inputs, current_inputs, ok)
                if not ok:
                    label, difference = "conformance_mismatch", ["Cargo oracle differs from native output"]
                if case.get("expected_status") and observed["status"] != case["expected_status"]:
                    label, difference = "regression", ["declared edge-case success/refusal expectation changed"]
                if case.get("frozen"):
                    if observed["status"] != "ok" or len(observed["profiles"]) != 100 or len(observed["version_union"]) != 186:
                        label, difference = "regression", ["frozen qualification did not reproduce 100 roots / 186 versions"]
                if case.get("lock") and plan:
                    lock_ids_expected = {(v["name"], v["version"], v["checksum"]) for s in plan["catalog"]["crate-locks"].values() for v in s["pins"]}
                    if {(v["name"], v["version"], v["cksum"]) for v in plan["versions"]} != lock_ids_expected:
                        label, difference = "regression", ["native lock union differs from original imported pins/checksums"]
                rows.append(dict(id=case["id"], corpus=case["corpus"], classification=label,
                                 differences=difference, native=observed, oracle=cargo, oracle_ok=ok,
                                 timings={"plan_seconds":elapsed}, refusal=error, input_sha256=inputs))
                if override is None:
                    observed_live.update(cache.files)
                if label == "upstream":
                    dump(work / "proposed-baseline" / "expected" / expected_file.name, dict(schema=1, created_at=datetime.date.today().isoformat(), source={"product_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO,text=True).strip(), "case":case["id"]}, expected=observed))
            except Exception as error:
                rows.append(dict(id=case["id"], corpus=case["corpus"], classification="infrastructure_failure", error=str(error)))
    guards = []
    if args.oracle:
        for guard in corpus(CORPUS / "tricky.json")["ci_guards"]:
            test = guard["test"]
            if not re.fullmatch(r"[a-z][a-z0-9_]+", test):
                raise ValueError("invalid CI test guard")
            p = subprocess.run(["cargo", "test", "--locked", "--offline", "--test", "crates_mirror", test, "--", "--exact"],
                               cwd=REPO, text=True, capture_output=True, timeout=180)
            (work / (test + ".log")).write_text(p.stdout + p.stderr)
            guards.append(dict(test=test, reason=guard["reason"], ok=p.returncode == 0 and "1 passed" in p.stdout))
    failed = any(r["classification"] not in ("clean", "upstream", "recorded") for r in rows) or any(not g["ok"] for g in guards)
    changed = any(r["classification"] == "upstream" for r in rows)
    if observed_live and (changed or args.record):
        save_bundle(work / "proposed-baseline/inputs/baseline.json.gz", baseline_inputs | observed_live)
    report = dict(schema=1, created_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  commit=subprocess.check_output(["git","rev-parse","HEAD"], cwd=REPO, text=True).strip(),
                  run_id=os.environ.get("GITHUB_RUN_ID"), event=os.environ.get("GITHUB_EVENT_NAME"),
                  corpus_sha256={str(p.relative_to(CORPUS)):digest(p.read_bytes()) for p in sorted(CORPUS.rglob("*")) if p.is_file()},
                  binary_sha256=digest(binary.read_bytes()), fast=args.fast, cargo_oracle=args.oracle,
                  failed=failed, changed=changed, cases=rows, guards=guards,
                  registry_requests=cache.requests, archive_requests=[],
                  frozen_historical_measured_archive_bytes=45125358,
                  limits="Archive sizes are unknown without archive HEAD/GET; sparse-only planning does not measure payload bytes.")
    dump(work / "report.json", report)
    text = ["# Crate corpus report", "", f"Commit: `{report['commit']}`", "",
            f"Result: {'FAIL' if failed else 'upstream differences' if changed else 'clean'}; Cargo oracle: {args.oracle}.", "",
            "| Case | Classification | Versions |", "| --- | --- | --- |"]
    for r in rows:
        text.append(f"| {r['id']} | {r['classification']} | {len(r.get('native',{}).get('version_union',[]))} |")
    text += ["", report["limits"], "", "Frozen 2026-11-13 control requires 100 roots and 186 versions.",
             "", "Upstream differences have a proposed-baseline artifact; refreshes require a reviewed PR, never auto-merge."]
    (work / "summary.md").write_text("\n".join(text) + "\n")
    (work / "baseline-update.md").write_text("# Proposed baseline-update PR\n\n" +
        ("Do not update baselines: resolve regression/conformance failures first.\n" if failed else
         "No baseline update is needed.\n" if not changed and not args.record else
         "Review every metadata delta in report.json. Require a FULL CI validation (without --record) before copying proposed-baseline into tests/corpus and opening a reviewed PR. Fast proposals are incomplete. Never auto-merge.\n"))
    patches=[]
    for p in sorted((work/"proposed-baseline/expected").glob("*.json")):
        original=CORPUS/"expected"/p.name
        patches.extend(difflib.unified_diff(original.read_text().splitlines(True) if original.exists() else [],
                         p.read_text().splitlines(True),fromfile="a/tests/corpus/expected/"+p.name,
                         tofile="b/tests/corpus/expected/"+p.name))
    (work/"baseline.patch").write_text("".join(patches))
    print(json.dumps({"failed":failed,"changed":changed,"cases":len(rows)}), flush=True)
    return int(failed)


def refresh(args):
    old = corpus(args.file)
    url = "https://crates.io/api/v1/crates?page=1&per_page=100&sort=downloads"
    version=tomllib.loads((REPO/"Cargo.toml").read_text())["package"]["version"]
    request = urllib.request.Request(url, headers={"User-Agent":f"rust-mirror/{version} (+https://github.com/rustcommons/rust-mirror; {CONTACT})"})
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    with opener.open(request, timeout=60) as response:
        data = response.read(4 * 1024 * 1024 + 1)
    if len(data) > 4 * 1024 * 1024:
        raise ValueError("ranking API response too large")
    previous = {r["name"]:r for r in old["roots"]}
    roots = []
    for c in json.loads(data)["crates"]:
        root = dict(name=c["id"], version=c.get("max_stable_version") or c["max_version"],
                    features=previous.get(c["id"],{}).get("features",[]),
                    default_features=previous.get(c["id"],{}).get("default_features",True))
        shard(root["name"])
        roots.append(root)
    names = {r["name"] for r in roots}
    if len(roots) != 100 or len(names) != 100:
        raise ValueError("ranking response must have 100 distinct roots")
    new = dict(schema=1, created_at=datetime.date.today().isoformat(),
               source=dict(url=url,response_sha256=digest(data)), roots=roots)
    diff = dict(schema=1, added_roots=sorted(names-set(previous)), removed_roots=sorted(set(previous)-names),
                changed_profiles=[dict(before=previous[r['name']],after=r) for r in roots if r['name'] in previous and r != previous[r['name']]])
    dump(args.report, diff)
    if args.write:
        dump(args.file, new)
    else:
        dump(Path(args.report).with_suffix('.proposal.json'), new)
    print(json.dumps(diff,indent=2))


ISSUE_TITLE = "Crate corpus report"
ISSUE_MARKER = "<!-- rust-mirror-crate-corpus -->"


def github_api(method, repository, endpoint, body=None):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("invalid GitHub repository")
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if not token:
        raise ValueError("GitHub token required for workflow API")
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request("https://api.github.com/repos/"+repository+"/"+endpoint,
        data=data,method=method,headers={"Authorization":"Bearer "+token,
        "Accept":"application/vnd.github+json","X-GitHub-Api-Version":"2022-11-28",
        "User-Agent":"rust-mirror-corpus (+"+CONTACT+")"})
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    with opener.open(request,timeout=60) as response:
        raw = response.read(8*1024*1024+1)
    if len(raw)>8*1024*1024:
        raise ValueError("GitHub response budget exceeded")
    return json.loads(raw) if raw else None


def track_report(repository, report, summary, api=github_api):
    matches=[]
    for page in range(1,11):
        items=api("GET",repository,f"issues?state=all&per_page=100&page={page}")
        matches.extend(i for i in items if i.get("title")==ISSUE_TITLE and "pull_request" not in i)
        if len(items)<100:
            break
    else:
        raise ValueError("issue pagination bound reached; refuse to create a possible duplicate")
    if len(matches)>1:
        raise ValueError("multiple corpus tracking issues exist; owner must reconcile them")
    issue=matches[0] if matches else None
    changed=bool(report.get("failed") or report.get("changed"))
    if not changed:
        if issue and issue["state"]=="open":
            api("PATCH",repository,"issues/"+str(issue["number"]),{"state":"closed"})
            return "closed"
        return "clean"
    body=ISSUE_MARKER+"\n"+summary[:55000]+"\n\nFull JSON, Cargo logs and baseline-update proposal are in the workflow artifacts. No automatic baseline merge."
    if issue:
        # Preserve human context outside our explicitly managed block.
        old=issue.get("body") or ""
        prefix=old.split(ISSUE_MARKER,1)[0] if ISSUE_MARKER in old else old+"\n\n"
        api("PATCH",repository,"issues/"+str(issue["number"]),{"state":"open","body":prefix+body})
        return "updated"
    api("POST",repository,"issues",{"title":ISSUE_TITLE,"body":body})
    return "opened"


def release_check(repository, sha, api=github_api):
    if not re.fullmatch(r"[0-9a-f]{40}",sha):
        raise ValueError("release corpus gate requires an exact commit")
    endpoint="actions/workflows/corpus.yml/runs?"
    weekly=api("GET",repository,endpoint+"event=schedule&branch=main&per_page=1")["workflow_runs"]
    if weekly and weekly[0]["status"]=="completed" and weekly[0]["conclusion"]=="success":
        return weekly[0]["id"]
    dispatch=api("GET",repository,endpoint+"event=workflow_dispatch&head_sha="+sha+"&per_page=100")["workflow_runs"]
    for run in dispatch:
        if run["head_sha"]==sha and run["status"]=="completed" and run["conclusion"]=="success":
            return run["id"]
    raise ValueError("corpus gate refused: latest weekly must pass with only clean/upstream classifications, or dispatch the full corpus on this release commit")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run")
    run.add_argument("--binary", type=Path, required=True)
    run.add_argument("--output", type=Path, required=True)
    run.add_argument("--cache", type=Path)
    run.add_argument("--fast", action="store_true")
    run.add_argument("--oracle", action="store_true")
    run.add_argument("--record", action="store_true", help="Propose expected results in output only; never modify committed baselines")
    update = sub.add_parser("refresh")
    update.add_argument("--file", type=Path, default=CORPUS/'top-crates.json')
    update.add_argument("--report", type=Path, required=True)
    update.add_argument("--write", action="store_true")
    tracking = sub.add_parser("track")
    tracking.add_argument("--report",type=Path,required=True)
    tracking.add_argument("--summary",type=Path,required=True)
    tracking.add_argument("--repository",required=True)
    gate = sub.add_parser("release-check")
    gate.add_argument("--repository",required=True)
    gate.add_argument("--sha",required=True)
    args = parser.parse_args()
    if args.command == "run":
        return run_suite(args)
    if args.command == "refresh":
        return refresh(args)
    if args.command == "track":
        report=load(args.report) if args.report.exists() else {"failed":True}
        summary=args.summary.read_text() if args.summary.exists() else "Corpus job produced no report; inspect the failed workflow."
        print(track_report(args.repository,report,summary))
    else:
        print("Corpus release gate passed with run",release_check(args.repository,args.sha))
    return 0


if __name__ == "__main__":
    sys.exit(main())
