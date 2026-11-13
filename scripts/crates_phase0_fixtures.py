"""Small synthetic sparse registries for independent Cargo conformance, not runtime code."""
import gzip
import hashlib
import http.server
import io
import json
from pathlib import Path
import subprocess
import tarfile
import threading
from check_crates_phase0 import cargo_environment, project, run


def dep(name, req="1", **kw):
    return dict(name=name, req=req, features=[], default_features=True, optional=False,
                target=None, kind="normal", registry=None) | kw


def record(name, version="1.0.0", deps=(), features=None, **kw):
    return dict(name=name, vers=version, deps=list(deps), features=features or {},
                cksum="0" * 64, yanked=False) | kw


def cases():
    common = [record("root", deps=[dep("left"), dep("right")])]
    yield "feature-intersection", common + [
        record("left", deps=[dep("x", features=["a"])]),
        record("right", deps=[dep("x", features=["b"])]),
        record("x", "1.3.0", features={"a": []}),
        record("x", "1.2.0", features={"b": []}),
        record("x", "1.1.0", features={"a": [], "b": []})], {}, True
    yield "prerelease-intersection", common + [
        record("left", deps=[dep("x", ">=1.0.0, <1.2.0-zzz")]),
        record("right", deps=[dep("x", ">=1.0.0, <1.3.0-zzz")]),
        *[record("x", v) for v in ("1.1.0", "1.2.0-alpha", "1.3.0-alpha")]], {}, True
    yield "links-backtrack", [
        record("root", deps=[dep("wrapper"), dep("sys-old")]),
        record("wrapper", "1.1.0", deps=[dep("sys-new")]),
        record("wrapper", deps=[dep("sys-old")]),
        record("sys-new", links="native"), record("sys-old", links="native")], {}, True
    yield "pin-forces-older", common + [
        record("left", deps=[dep("x", "1")]),
        record("right", deps=[dep("x", "=1.0.0")]),
        record("x"), record("x", "1.1.0")], {}, True
    yield "exact-conflict", common + [
        record("left", deps=[dep("x", "=1.0.0")]),
        record("right", deps=[dep("x", "=1.1.0")]),
        record("x"), record("x", "1.1.0")], {}, False
    weak = [record("root", deps=[dep("alias", package="actual", optional=True,
                    default_features=False)], features={"default": ["weak"],
                    "weak": ["alias?/forward"]}, v=2, features2={"activate": ["dep:alias"]}),
            record("actual", deps=[dep("leaf", optional=True)],
                   features={"forward": ["dep:leaf"]}), record("leaf")]
    yield "weak-lock-only", weak, {}, True
    yield "weak-disabled", weak, {"default_features": False}, True
    yield "renamed-strong-v2", weak, {"features": ["activate"]}, True
    yield "targets-build-dev", [record("root", deps=[
        dep("windows-build", kind="build", target="cfg(windows)"),
        dep("unused-dev", kind="dev")]), record("windows-build")], {}, True
    yield "yanked-pin", [record("root", yanked=True)], {}, False


def archive(entry, fail_compile=False):
    name, version = entry["name"], entry["vers"]
    lines = ["[package]", f'name={json.dumps(name)}', f'version={json.dumps(version)}',
             'edition="2021"']
    if entry.get("links"):
        lines += [f'links={json.dumps(entry["links"])}', 'build="build.rs"']
    tables = {}
    for dep in entry["deps"]:
        # Dependencies' dev edges are deliberately absent from the public fixture
        # manifest: mirrors do not claim testing published dependency packages.
        if dep["kind"] == "dev":
            continue
        kind = "build-dependencies" if dep["kind"] == "build" else "dependencies"
        table = f'target.{json.dumps(dep["target"])}.{kind}' if dep["target"] else kind
        value = dict(version=dep["req"], features=dep["features"],
                     optional=dep["optional"], **{"default-features": dep["default_features"]})
        if dep.get("package"):
            value["package"] = dep["package"]
        tables.setdefault(table, []).append(json.dumps(dep["name"]) + "={" +
            ",".join(k + "=" + json.dumps(v) for k, v in value.items()) + "}")
    for table, values in tables.items():
        lines += [f"[{table}]", *values]
    features = dict(entry["features"]) | entry.get("features2", {})
    if features:
        lines += ["[features]", *[json.dumps(k) + "=" + json.dumps(v) for k, v in features.items()]]
    files = {"Cargo.toml": "\n".join(lines) + "\n", "src/lib.rs":
             'compile_error!("inactive weak dependency must not build");\n' if fail_compile else "pub fn fixture() {}\n"}
    if entry.get("links"):
        files["build.rs"] = "fn main() {}\n"
    tar = io.BytesIO()
    with tarfile.open(fileobj=tar, mode="w", format=tarfile.USTAR_FORMAT) as output:
        for path, content in files.items():
            data = content.encode()
            info = tarfile.TarInfo(f"{name}-{version}/{path}")
            info.size, info.mode, info.mtime = len(data), 0o644, 0
            output.addfile(info, io.BytesIO(data))
    return gzip.compress(tar.getvalue(), mtime=0)


def check_fixtures(driver, output):
    reports = []
    for label, entries, profile, success in cases():
        work = output / label
        work.mkdir(parents=True)
        cache = work / "index"
        archives = {}
        full = {}
        for entry in entries:
            data = archive(entry, fail_compile=(label == "weak-lock-only" and entry["name"] == "actual"))
            archives[(entry["name"], entry["vers"])] = data
            entry["cksum"] = hashlib.sha256(data).hexdigest()
            name = entry["name"]
            shard = "1/" + name if len(name) == 1 else "2/" + name if len(name) == 2 else "3/" + name[0] + "/" + name if len(name) == 3 else name[:2] + "/" + name[2:4] + "/" + name
            full[shard] = full.get(shard, b"") + (json.dumps(entry) + "\n").encode()
        for path, data in full.items():
            file = cache / path
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(data)
        root = dict(name="root", version="1.0.0") | profile
        roots = work / "roots.json"
        roots.write_text(json.dumps([root]))
        plan_file = work / "plan.json"
        result = subprocess.run([str(driver), str(roots), str(cache), str(plan_file), "--offline"],
                                capture_output=True, text=True, timeout=60)
        (work / "native.log").write_text(result.stdout + result.stderr)
        if (result.returncode == 0) != success:
            raise AssertionError(f"native fixture {label}: {result.stderr}")
        filtered = {}
        if success:
            plan = json.loads(plan_file.read_text())
            assert plan["archive_requests"] == 0
            wanted = {(v["name"], v["version"]) for v in plan["versions"]}
            for path, data in full.items():
                selected = [line for line in data.splitlines(keepends=True)
                            if (json.loads(line)["name"], json.loads(line)["vers"]) in wanted]
                if selected:
                    filtered[path] = b"".join(selected)
        transferred = []
        allow_archives = False

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                parts = self.path.strip("/").split("/")
                mode, path = parts[0], "/".join(parts[1:])
                if mode == "archive":
                    transferred.append(path)
                    data = archives.get(tuple(parts[1:])) if allow_archives else None
                elif path == "config.json":
                    data = json.dumps({"dl": base + "/archive/{crate}/{version}"}).encode()
                else:
                    data = (full if mode == "full" else filtered).get(path)
                if data is None:
                    self.send_error(404)
                    return
                self.send_response(200)
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        base = f"http://127.0.0.1:{server.server_port}"
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            for mode in ("full", "filtered") if success else ("full",):
                path = work / mode
                project(path, root, base, mode)
                env = cargo_environment(work / ("home-" + mode))
                command = ["cargo", "generate-lockfile"]
                if success:
                    log = run(command, cwd=path, env=env)
                else:
                    failed = subprocess.run(command, cwd=path, env=env, capture_output=True, text=True, timeout=60)
                    assert failed.returncode != 0, label
                    log = failed.stdout + failed.stderr
                (work / (mode + ".log")).write_text(log)
                assert not transferred, "archive requested during planning/lock generation"
                if mode == "filtered":
                    allow_archives = True
                    run(["cargo", "fetch", "--locked"], cwd=path, env=env)
                    run(["cargo", "build", "--locked", "--offline"], cwd=path, env=env)
            reports.append(dict(case=label, expected_success=success,
                                cargo_conformance=True, fixture_archive_requests=len(transferred)))
            print(json.dumps(reports[-1]), flush=True)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=10)
    (output / "fixtures.json").write_text(json.dumps(reports, indent=2) + "\n")
    return reports
