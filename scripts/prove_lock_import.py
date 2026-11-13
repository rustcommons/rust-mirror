#!/usr/bin/env python3
"""Pinned real-lock HTTPS Cargo qualification. Uses private homes; never publishes remotely."""
import argparse
import hashlib
import http.server
import io
import json
import os
from pathlib import Path
import shutil
import ssl
import subprocess
import tarfile
import threading
import tomllib
import urllib.request
from prove_crates_https import cargo_paths, certificates, command, environment


def invoke(binary, args, evidence, name):
    result = command([binary, "--events", "jsonl", "crates", *args], timeout=600)
    (evidence / (name + ".jsonl")).write_text(result.stdout)
    (evidence / (name + ".stderr")).write_text(result.stderr)
    events = [json.loads(line) for line in result.stdout.splitlines()]
    assert events[0]["type"] == "hello" and events[0]["protocol"] == 1
    assert events[0]["command"] == "crates-" + str(args[0])
    assert events[-1]["type"] == "result" and events[-1]["ok"]
    assert [e["seq"] for e in events] == list(range(len(events)))
    assert sum(e["type"] == "result" for e in events) == 1
    return events[-1]["report"]


def sources(root, corpus):
    projects = root / "projects"
    projects.mkdir()
    for entry in corpus:
        request = urllib.request.Request(f'https://codeload.github.com/{entry["repo"]}/tar.gz/{entry["commit"]}',
                                        headers={"User-Agent": "rust-mirror-lock-qualification (https://github.com/rustcommons/rust-mirror)"})
        with urllib.request.urlopen(request, timeout=60) as response:
            data = response.read(64 * 1024 * 1024 + 1)
        assert len(data) <= 64 * 1024 * 1024
        destination = projects / entry["name"]
        destination.mkdir()
        skipped = []
        total = 0
        with tarfile.open(fileobj=io.BytesIO(data)) as archive:
            members = archive.getmembers()
            assert len(members) <= 50000
            for member in members:
                parts = Path(member.name).parts[1:]
                assert not member.name.startswith("/") and ".." not in parts
                if not parts:
                    continue
                if member.issym() or member.islnk():
                    skipped.append("/".join(parts))
                    continue
                path = destination.joinpath(*parts)
                if member.isdir():
                    path.mkdir(parents=True, exist_ok=True)
                else:
                    assert member.isfile()
                    total += member.size
                    assert total <= 512 * 1024 * 1024
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(archive.extractfile(member).read())
        assert sorted(skipped) == sorted(entry["unextracted_links"])
        assert hashlib.sha256((destination / "Cargo.lock").read_bytes()).hexdigest() == entry["lock_sha256"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--reuse", type=Path, help="Owned prepared source/mirror evidence for a local client-only rerun")
    args = parser.parse_args()
    root = args.evidence.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    corpus = json.loads((Path(__file__).resolve().parents[1] / "tests/fixtures/crates-lock-corpus.json").read_text())["projects"]
    cargo, rustc = cargo_paths()
    if args.reuse:
        prepared = args.reuse.resolve()
        mirror = prepared / "run/mirror"
    else:
        prepared = root
        sources(root, corpus)
        config = root / "catalog.toml"
        host = next(line.split(": ", 1)[1] for line in command([rustc, "-vV"]).stdout.splitlines() if line.startswith("host: "))
        config.write_text('[crates]\n[crate-policy]\nhosts=['+json.dumps(host)+']\ntargets=['+json.dumps(host)+']\n[crate-output]\nbase-url="https://localhost:18443/lock-generation"\n[crate-network]\ncontact="https://github.com/rustcommons/rust-mirror/issues"\n')
        original = config.read_bytes()
        options = ["import-lock", "--config", config]
        for entry in corpus:
            options.extend(["--lock", entry["name"] + "=" + str(root / "projects" / entry["name"] / "Cargo.lock")])
        preview = invoke(binary, options, root, "import-preview")
        assert preview["accepted"] and not preview["written"] and config.read_bytes() == original
        imported = invoke(binary, [*options, "--write"], root, "import-write")
        assert imported["written"] and imported["external_sources_required"]
        assert str(root) not in config.read_text() and "git+https://" not in config.read_text()
        assert [s["lock_version"] for s in imported["sources"]] == [e["version"] for e in corpus]
        plan_path = root / "plan.json"
        plan = invoke(binary, ["plan", "--config", config, "--output", plan_path], root, "plan")
        expected = {(p["name"], p["version"], p["checksum"]) for s in imported["catalog"]["crate-locks"].values() for p in s["pins"]}
        assert {(v["name"], v["version"], v["cksum"]) for v in plan["versions"]} == expected
        assert any(v.get("locked_yanked") for v in plan["versions"])
        planning_events=[json.loads(line) for line in (root/"plan.jsonl").read_text().splitlines()]
        assert not any(e.get("type")=="artifact" and e.get("path","").endswith(".crate") and e.get("action")!="head" for e in planning_events)
        # The backend can replay/fetch after every original lockfile has disappeared.
        moved = []
        try:
            for entry in corpus:
                path = root / "projects" / entry["name"] / "Cargo.lock"
                path.rename(path.with_name("Cargo.lock.imported-original"))
                moved.append(path)
            invoke(binary, ["show", "--plan", plan_path], root, "show-without-originals")
            invoke(binary, ["fetch", "--plan", plan_path, "--output", root / "run", "--approve-plan", plan["plan_id"]], root, "fetch")
        finally:
            for path in moved:
                path.with_name("Cargo.lock.imported-original").rename(path)
        mirror = root / "run/mirror"
    inventory = tomllib.loads((mirror / "snapshot.toml").read_text())
    assert inventory["schema"] == 5 and inventory["plan"]["schema"] == 2
    base = inventory["plan"]["catalog"]["crate-output"]["base-url"].rstrip("/")
    assert base == "https://localhost:18443/lock-generation"
    requests = []
    fault = {"mode": None}

    class Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *a, **kw):
            super().__init__(*a, directory=str(mirror), **kw)

        def log_message(self, *a):
            pass

        def do_GET(self):
            requests.append({"path": self.path, "fault": fault["mode"]})
            if not self.path.startswith("/lock-generation/") or "?" in self.path:
                self.send_error(404)
                return
            if self.path.endswith(".crate") and fault["mode"]:
                if fault["mode"] == "missing":
                    self.send_error(404)
                else:
                    data = b"corrupt-lock-payload"
                    self.send_response(200)
                    self.send_header("Content-Length", str(len(data)))
                    self.end_headers()
                    self.wfile.write(data)
                return
            self.path = self.path.removeprefix("/lock-generation")
            super().do_GET()

        def list_directory(self, path):
            self.send_error(404)
            return None

    reports = []
    with certificates() as pki:
        ca_file = root / "test-ca.pem"
        shutil.copyfile(pki / "ca.pem", ca_file)
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 18443), Handler)
        server.daemon_threads = True
        tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        tls.minimum_version = ssl.TLSVersion.TLSv1_2
        tls.load_cert_chain(pki / "server.pem", pki / "server.key")
        server.socket = tls.wrap_socket(server.socket, server_side=True)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            for entry in corpus:
                case = root / entry["name"]
                case.mkdir()
                project = prepared / "projects" / entry["name"]
                if not project.exists():
                    project = prepared / entry["name"]
                original_lock = (project / "Cargo.lock").read_bytes()
                home = case / "cargo-home"
                home.mkdir()
                configuration = '[source.crates-io]\nreplace-with="lock-proof"\n[source.lock-proof]\nregistry=' + json.dumps("sparse+"+base+"/cargo/index/") + '\n[http]\nssl-verify=true\ncainfo=' + json.dumps(str(ca_file)) + '\n'
                (home / "config.toml").write_text(configuration)
                env = environment(home, case / "target", rustc)
                cases = []

                def run(name, options, success=True, use_home=home):
                    result = command([cargo, *options], cwd=project, env=environment(use_home, case / "target", rustc), success=success, timeout=420)
                    (case / (name+".stdout")).write_text(result.stdout)
                    (case / (name+".stderr")).write_text(result.stderr)
                    assert (project / "Cargo.lock").read_bytes() == original_lock, "Cargo changed a locked historical input"
                    cases.append({"case": name, "exit": result.returncode, "expected_success": success})
                    return result

                mixed = "rust-analyzer" in entry["name"]
                if mixed:
                    run("provision-git-independently", ["fetch", "--locked"])
                    assert (home / "git").exists()
                    # Keep only explicitly provisioned Git checkouts, not registry payloads.
                    shutil.rmtree(home / "registry")
                run("mirror-fetch-locked", ["fetch", "--locked"])
                run("offline-build-locked", ["build", "--locked", "--offline", *entry["build_args"]])
                if mixed:
                    cold = case / "missing-git-home"
                    cold.mkdir()
                    (cold / "config.toml").write_text(configuration)
                    shutil.copytree(home / "registry", cold / "registry")
                    failed = run("missing-git-offline-refusal", ["fetch", "--locked", "--offline"], False, cold)
                    assert "chalk" in failed.stderr.lower()
                if not reports:
                    for mode in ["missing", "corrupt"]:
                        cold = case / (mode+"-home")
                        cold.mkdir()
                        (cold / "config.toml").write_text(configuration)
                        fault["mode"] = mode
                        result = run(mode+"-archive-refusal", ["fetch", "--locked"], False, cold)
                        assert any(r["fault"] == mode and r["path"].endswith(".crate") for r in requests)
                        if mode == "corrupt":
                            assert "checksum" in result.stderr.lower()
                    fault["mode"] = None
                reports.append({**entry, "cases": cases, "registry_cache_initially_empty": True, "git_provisioned_separately": mixed})
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=10)
            (root / "requests.json").write_text(json.dumps(requests, indent=2)+"\n")
            shutil.copyfile(pki / "crl-requests.json", root / "revocation-requests.json")
    summary = {"schema": 1, "backend_source": subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(), "cargo": command([cargo,"--version"]).stdout.strip(), "rustc": command([rustc,"-vV"]).stdout, "plan_id": inventory["plan"]["plan_id"], "versions": inventory["plan"]["counts"]["versions"], "planning_archive_gets": 0, "archive_version_overfetch": 0, "archive_bytes": sum(f["bytes"] for f in inventory["files"] if f["path"].endswith(".crate")), "locked_yanked": [{"name":v["name"],"version":v["version"]} for v in inventory["plan"]["versions"] if v.get("locked_yanked")], "projects": reports, "scope": "Exact registry pins only; git/path sources supplied independently. Builds are the named packages, not arbitrary projects/targets/features.", "private_tls_keys_destroyed": True}
    (root / "summary.json").write_text(json.dumps(summary,indent=2)+"\n")
    print(json.dumps(summary), flush=True)


if __name__ == "__main__":
    main()
