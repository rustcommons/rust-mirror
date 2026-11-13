#!/usr/bin/env python3
"""Research-only Cargo conformance oracle; never used by the runtime library.

Uses private Cargo homes and a loopback sparse fixture. Planning is performed by
an already-built native example, which cannot download archives. Full-index Cargo
reference resolution may request additional official index metadata, never payloads.
"""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import threading
import tomllib
import urllib.error
import urllib.request

ROOTS = [
    dict(name="tokio", version="1.48.0", features=["full"]),
    dict(name="serde", version="1.0.228", features=["derive"]),
    dict(name="clap", version="4.5.50", features=["derive"]),
    dict(name="reqwest", version="0.12.24"),
    dict(name="rand", version="0.8.5"),
    dict(name="rand", version="0.9.2"),
    dict(name="openssl-sys", version="0.9.110"),
    dict(name="syn", version="2.0.110", features=["full", "visit", "fold", "extra-traits"]),
]


def run(command, cwd=None, env=None):
    result = subprocess.run(command, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, timeout=600)
    if result.returncode:
        raise RuntimeError(f"{command[0]} failed ({result.returncode}):\n{result.stdout[-16000:]}")
    return result.stdout


def cargo_environment(home):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("CARGO_", "RUSTFLAGS", "RUSTDOCFLAGS", "RUSTC_WRAPPER"))}
    env.update(CARGO_HOME=str(home), CARGO_NET_RETRY="0", CARGO_HTTP_PROXY="",
               CARGO_BUILD_JOBS="2", CARGO_TERM_COLOR="never")
    return env


def project(path, root, base, mode):
    path.mkdir(parents=True)
    (path / "src").mkdir()
    (path / "src/lib.rs").write_text("pub fn fixture() {}\n")
    (path / ".cargo").mkdir()
    (path / ".cargo/config.toml").write_text(
        '[source.crates-io]\nreplace-with="fixture"\n[source.fixture]\n'
        f'registry="sparse+{base}/{mode}/"\n')
    dep = dict(version="=" + root["version"], package=root["name"],
               features=root.get("features", []),
               **{"default-features": root.get("default_features", True)})
    # JSON values are valid TOML inline-table values for these simple field types.
    spec = ", ".join(f"{k}={json.dumps(v)}" for k, v in dep.items())
    (path / "Cargo.toml").write_text(
        '[package]\nname="phase0-probe"\nversion="0.0.0"\nedition="2021"\n'
        'resolver="2"\n[dependencies]\nselected={' + spec + '}\n')


def lock_ids(path):
    lock = tomllib.loads((path / "Cargo.lock").read_text())
    return {p["name"] + "@" + p["version"] for p in lock["package"] if "source" in p}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--driver", required=True, help="Already-built native crates_phase0 example")
    parser.add_argument("--work", required=True, type=Path, help="New private output directory")
    parser.add_argument("--cache", type=Path, help="Explicit metadata cache, never a Cargo home")
    parser.add_argument("--roots", type=Path, help="Explicit frozen exact-root JSON corpus; default is the original Phase 0 corpus")
    parser.add_argument("--frozen-plan", type=Path, help="Validate an already generated MVP frozen plan against both Cargo indexes")
    args = parser.parse_args()
    roots = json.loads(args.roots.read_text()) if args.roots else ROOTS
    args.work.mkdir(parents=True, exist_ok=False)
    args.work = args.work.resolve()
    cache = args.cache.resolve() if args.cache else args.work / "index"
    cache.mkdir(parents=True, exist_ok=True)
    roots_file = args.work / "roots.json"
    roots_file.write_text(json.dumps(roots, indent=2) + "\n")
    plan_file = args.work / "plan.json"
    if args.frozen_plan:
        frozen = json.loads(args.frozen_plan.read_text())
        assert frozen["schema"] in (1, 2) and not frozen["previous_inventory_sha256"]
        expected = {(r["name"], r["version"]) for r in roots}
        profiles = [r["profile"] for r in frozen["roots"]]
        assert len(profiles) == len(roots) and {(r["name"], r["version"]) for r in profiles} == expected
        roots = profiles  # Native witness/reason indexes follow catalog alias ordering.
        roots_file.write_text(json.dumps(roots, indent=2) + "\n")
        plan = dict(frozen, metadata_bytes=sum(o["bytes"] for o in frozen["observations"]), archive_requests=0)
        plan["versions"] = [dict(v, index_line=list(v["index_line"].encode())) for v in frozen["versions"]]
        plan_file.write_text(json.dumps(plan, indent=2) + "\n")
    else:
        print(run([str(Path(args.driver).resolve()), str(roots_file), str(cache), str(plan_file)]), flush=True)
        plan = json.loads(plan_file.read_text())
    assert plan["archive_requests"] == 0
    filtered = {}
    native = set()
    for version in plan["versions"]:
        native.add(version["name"] + "@" + version["version"])
        name = version["name"].lower()
        path = ("1/" + name if len(name) == 1 else "2/" + name if len(name) == 2 else
                "3/" + name[0] + "/" + name if len(name) == 3 else name[:2] + "/" + name[2:4] + "/" + name)
        filtered[path] = filtered.get(path, b"") + bytes(version["index_line"])
    # Preserve upstream line ordering, rather than sorting JSON records by version.
    for path in filtered:
        if args.frozen_plan:
            continue  # FrozenPlan already preserves source ordering and is validated by the native loader.
        data = (cache / path).read_bytes()
        filtered[path] = b"".join(line for line in data.splitlines(keepends=True)
            if json.loads(line)["name"] + "@" + json.loads(line)["vers"] in native)
    archive_requests = []
    metadata_hashes = {}
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                                        # Official sparse origin must not redirect.
                                        type("NoRedirect", (urllib.request.HTTPRedirectHandler,),
                                             {"redirect_request": lambda *a, **kw: None})())

    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def do_GET(self):
            parts = self.path.strip("/").split("/")
            mode, path = parts[0], "/".join(parts[1:])
            if mode not in ("filtered", "full") or not path or ".." in parts or "?" in path:
                self.send_error(404)
                return
            if path == "config.json":
                data = json.dumps({"dl": base + "/archive/{crate}/{version}"}).encode()
            elif mode == "filtered":
                data = filtered.get(path)
                if data is None:
                    self.send_error(404)
                    return
            else:
                file = cache / path
                if not file.exists():
                    # Only source metadata; no crate payload operation exists here.
                    try:
                        request = urllib.request.Request("https://index.crates.io/" + path,
                            headers={"User-Agent": "rust-mirror-phase0-conformance (+https://github.com/rustcommons/rust-mirror)"})
                        with opener.open(request, timeout=60) as response:
                            data = response.read(32 * 1024 * 1024 + 1)
                        if len(data) > 32 * 1024 * 1024:
                            raise ValueError("index limit")
                        file.parent.mkdir(parents=True, exist_ok=True)
                        file.write_bytes(data)
                    except urllib.error.HTTPError as error:
                        self.send_error(error.code)
                        return
                data = file.read_bytes()
                metadata_hashes[path] = hashlib.sha256(data).hexdigest()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

    # Archive requests cannot hide behind the index handler's mode refusal.
    original = Handler.do_GET
    def get(handler):
        if handler.path.startswith("/archive/"):
            archive_requests.append(handler.path)
            handler.send_error(403, "archives forbidden during planning/conformance lock resolution")
        else:
            original(handler)
    Handler.do_GET = get
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    base = f"http://127.0.0.1:{server.server_port}"
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    rows = []
    union = set()
    try:
        for n, root in enumerate(roots):
            chosen = {}
            for mode in ("full", "filtered"):
                path = args.work / f"{n}-{mode}"
                project(path, root, base, mode)
                home = args.work / f"home-{n}-{mode}"
                output = run(["cargo", "generate-lockfile", "--manifest-path", str(path / "Cargo.toml")],
                             cwd=path, env=cargo_environment(home))
                (args.work / f"{n}-{mode}.log").write_text(output)
                chosen[mode] = lock_ids(path)
                if mode == "filtered" and not chosen[mode] <= native:
                    raise AssertionError("Cargo resolved a version outside native output")
            union |= chosen["full"]
            witness = set(plan["witnesses"][n]["versions"])
            # Reasons include conservative extras; count coverage per independent profile.
            coverage = {v["name"] + "@" + v["version"] for v in plan["versions"]
                        if any(r["root"] == n for r in v["reasons"])}
            row = dict(root=root, cargo_versions=len(chosen["full"]),
                       native_versions=len(coverage), witness_versions=len(witness),
                       filtered_cargo_versions=len(chosen["filtered"]),
                       native_only=sorted(coverage - chosen["full"]),
                       cargo_only=sorted(chosen["full"] - coverage))
            rows.append(row)
            print(json.dumps(row), flush=True)
    finally:
        server.shutdown()
        server.server_close()
        worker.join(timeout=10)
    assert not archive_requests, archive_requests
    shared_observations = []
    native_only_observations = []
    if args.frozen_plan:
        for observation in frozen["observations"]:
            name = observation["name"].lower()
            shard = ("1/" + name if len(name) == 1 else "2/" + name if len(name) == 2 else
                     "3/" + name[0] + "/" + name if len(name) == 3 else name[:2] + "/" + name[2:4] + "/" + name)
            file = cache / shard
            if not file.is_file():
                native_only_observations.append(observation["name"])
                continue
            raw = file.read_bytes()
            assert len(raw) == observation["bytes"] and hashlib.sha256(raw).hexdigest() == observation["sha256"], "upstream metadata changed between native planning and Cargo oracle; replan rather than compare different observations"
            shared_observations.append(observation["name"])
    source = Path(__file__).resolve().parent.parent
    source_hashes = {str(path.relative_to(source)): hashlib.sha256(path.read_bytes()).hexdigest()
                     for path in [source / "src/crates/mod.rs", source / "Cargo.toml", source / "Cargo.lock",
                                  source / "examples/crates_phase0.rs", Path(__file__).resolve(),
                                  source / "scripts/crates_phase0_fixtures.py"]}
    report = dict(source_sha256=source_hashes, cargo=run(["cargo", "--version"]).strip(), roots=rows,
                  native_versions=len(native), cargo_union_versions=len(union),
                  intersection=len(native & union), native_only=sorted(native - union),
                  cargo_only=sorted(union - native),
                  overfetch_percent=100 * len(native - union) / len(union),
                  payload_byte_overfetch=None, archive_requests=archive_requests,
                  metadata_bytes=plan["metadata_bytes"], index_sha256=metadata_hashes,
                  shared_index_observations_verified=shared_observations,
                  native_only_index_observations=native_only_observations)
    (args.work / "conformance.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k not in ("roots", "index_sha256")}), flush=True)
    from crates_phase0_fixtures import check_fixtures
    check_fixtures(Path(args.driver).resolve(), args.work / "fixtures")


if __name__ == "__main__":
    main()
