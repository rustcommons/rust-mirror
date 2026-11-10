#!/usr/bin/env python3
"""Linux-only real Rustup proof. Run inside a fresh network namespace as root.

This uses an HTTP PUT/GET fixture, NOT a real Nexus server. Only loopback exists.
"""
import argparse
import functools
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import threading
from urllib.parse import urlsplit
from urllib.request import urlopen
import publish_raw

PREFIX = "/repository/rust-offline/proof"


def prove(snapshot, evidence, selected_host=None, selection_name=None, additional_selections=None):
    snapshot = snapshot.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    result = {"status": "running", "server": "local HTTP PUT/GET contract fixture; not Nexus", "commands": []}
    server = None
    worker = None
    request_log = []
    try:
        subprocess.run(["ip", "link", "set", "lo", "up"], check=True)
        routes = json.loads(subprocess.check_output(["ip", "-json", "route"]))
        addresses = json.loads(subprocess.check_output(["ip", "-json", "address"]))
        if routes or {a["ifname"] for a in addresses} != {"lo"}:
            raise RuntimeError("proof requires a network namespace with only loopback and no routes")
        result["network"] = {"routes": routes, "interfaces": addresses}
        try:
            with socket.create_connection(("1.1.1.1", 443), timeout=1):
                raise RuntimeError("external network unexpectedly reachable")
        except OSError as error:
            result["external_connection_refused"] = type(error).__name__
        document, files = publish_raw.local_files(snapshot)
        expected = {n: sha for n, path, sha in files}
        collection_records = document.get("toolchains", []) if document["schema"] == 3 else []
        if additional_selections and not collection_records:
            raise ValueError("--also-selection requires a collection snapshot")
        if len(set(additional_selections or [])) != len(additional_selections or []):
            raise ValueError("duplicate additional selection")
        if document["schema"] == 3:
            matches = [r for r in document["toolchains"] if r["selection"]["name"] == selection_name]
            if len(matches) != 1:
                raise ValueError("collection proof needs --selection naming exactly one retained record")
            document = {**matches[0], "schema": 2, "rustup_version": document["rustup_version"]}
        elif selection_name is not None:
            raise ValueError("--selection is only for collection snapshots")
        release, rustup_version = document["release"], document["rustup_version"]
        if document["schema"] == 1:
            host = document["host"]
            if selected_host is not None and selected_host != host:
                raise ValueError("host not included in snapshot")
            profile, components, targets, install_label = "minimal", [], [], release
        else:
            selection = document["selection"]
            hosts = selection["hosts"]
            host = selected_host or (hosts[0] if len(hosts) == 1 else None)
            if host not in hosts:
                raise ValueError("select one explicit host included in a multi-host snapshot")
            profile, components, targets = selection["profile"], selection.get("components", []), selection.get("targets", [])
            install_label = document["pinned_toolchain"]
        if host != "x86_64-unknown-linux-gnu":
            raise ValueError("native proof currently qualifies only the x86_64 Linux GNU host")
        result["selection"] = {"host": host, "toolchain": install_label, "profile": profile, "components": components, "targets": targets}
        install_args = ["toolchain", "install", install_label, "--profile", profile, "--no-self-update"]
        if components:
            install_args.append("--component=" + ",".join(components))
        if targets:
            install_args.append("--target=" + ",".join(targets))
        os.environ.pop("NEXUS_USERNAME", None)
        os.environ.pop("NEXUS_PASSWORD", None)
        served = Path(tempfile.mkdtemp(prefix="rust-mirror-raw-fixture-"))

        class Handler(http.server.SimpleHTTPRequestHandler):
            def log_request(self, code="-", size="-"):
                request_log.append({"method": self.command, "path": self.path, "status": str(code)})

            def log_message(self, *args):
                pass

            def do_PUT(self):
                path = urlsplit(self.path).path
                if not path.startswith(PREFIX + "/") or any(p in (".", "..") for p in path.split("/")):
                    self.send_error(400)
                    return
                length = int(self.headers.get("Content-Length", "-1"))
                if not 0 <= length <= 512 * 1024 * 1024:
                    self.send_error(413)
                    return
                dest = served / path.lstrip("/")
                dest.parent.mkdir(parents=True, exist_ok=True)
                try:
                    with dest.open("xb") as output:
                        while length:
                            chunk = self.rfile.read(min(length, 1024 * 1024))
                            if not chunk:
                                raise RuntimeError("incomplete PUT")
                            output.write(chunk)
                            length -= len(chunk)
                except FileExistsError:
                    self.send_error(412)
                    return
                self.send_response(201)
                self.send_header("Content-Length", "0")
                self.end_headers()

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Handler, directory=str(served)))
        server.daemon_threads = True
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        base = f"http://127.0.0.1:{server.server_port}{PREFIX}"
        result["published"] = publish_raw.publish(snapshot, base, allow_loopback_http=True)
        client_request_start = len(request_log)
        bootstrap_path = f"rustup/archive/{rustup_version}/{host}/rustup-init"
        bootstrap = evidence / "rustup-init"
        with urlopen(base + "/" + bootstrap_path, timeout=10) as source, bootstrap.open("xb") as output:
            shutil.copyfileobj(source, output, 1024 * 1024)
        if publish_raw.digest(bootstrap) != expected[bootstrap_path]:
            raise RuntimeError("bootstrap changed during hosting")
        bootstrap.chmod(0o700)

        def invoke(name, args, env, cwd=None, success=True):
            completed = subprocess.run([str(x) for x in args], env=env, cwd=cwd, capture_output=True, timeout=180)
            (evidence / (name + ".stdout")).write_bytes(completed.stdout)
            (evidence / (name + ".stderr")).write_bytes(completed.stderr)
            result["commands"].append({"case": name, "exit": completed.returncode, "args": [str(x) for x in args]})
            if (completed.returncode == 0) != success:
                raise RuntimeError(f"unexpected command result: {name}; see recorded logs")
            return completed

        def client(name):
            client_root = evidence / name
            client_root.mkdir()
            home = client_root / "home"
            home.mkdir()
            env = {"PATH": "/usr/bin:/bin", "HOME": str(home), "CARGO_HOME": str(client_root / "cargo"), "RUSTUP_HOME": str(client_root / "rustup"), "RUSTUP_DIST_SERVER": base, "RUSTUP_UPDATE_ROOT": base + "/rustup", "RUSTUP_INIT_SKIP_PATH_CHECK": "yes", "RUSTUP_INIT_SKIP_SUDO_CHECK": "yes", "RUSTUP_MAX_RETRIES": "0", "RUSTUP_IO_THREADS": "2", "RUSTUP_UNPACK_RAM": "268435456", "CARGO_BUILD_JOBS": "2"}
            invoke(name + "-bootstrap", [bootstrap, "-y", "--no-modify-path", "--default-host", host, "--default-toolchain", "none"], env)
            rustup = client_root / "cargo/bin/rustup"
            invoke(name + "-disable-self-update", [rustup, "set", "auto-self-update", "disable"], env)
            return client_root, env, rustup

        client_root, env, rustup = client("positive")
        invoke("positive-install", [rustup, *install_args], env)
        versions = invoke("rustc-version", [rustup, "run", install_label, "rustc", "--version"], env)
        if not versions.stdout.startswith(f"rustc {release} ".encode()):
            raise RuntimeError("unexpected installed compiler")
        invoke("cargo-version", [rustup, "run", install_label, "cargo", "--version"], env)
        invoke("rustup-version", [rustup, "--version"], env)
        project = client_root / "hello"
        (project / "src").mkdir(parents=True)
        (project / "Cargo.toml").write_text('[package]\nname = "offline-proof"\nversion = "0.1.0"\nedition = "2021"\n')
        (project / "src/main.rs").write_text('fn main() {\n    println!("offline mirror proof ok");\n}\n')
        invoke("cargo-offline-build", [rustup, "run", install_label, "cargo", "build", "--offline"], env, cwd=project)
        run = invoke("run-built-program", [project / "target/debug/offline-proof"], env)
        if run.stdout != b"offline mirror proof ok\n":
            raise RuntimeError("unexpected program output")
        installed = invoke("installed-components", [rustup, "component", "list", "--installed", "--toolchain", install_label], env)
        installed_targets = invoke("installed-targets", [rustup, "target", "list", "--installed", "--toolchain", install_label], env)
        for component in components:
            if component.removesuffix("-preview").encode() not in installed.stdout:
                raise RuntimeError("requested component missing from installed list")
        for target in targets:
            if target.encode() not in installed_targets.stdout.splitlines():
                raise RuntimeError("requested target missing from installed list")
        if b"rustfmt" in installed.stdout:
            invoke("cargo-fmt", [rustup, "run", install_label, "cargo", "fmt", "--check"], env, cwd=project)
        if b"clippy" in installed.stdout:
            invoke("cargo-clippy", [rustup, "run", install_label, "cargo", "clippy", "--offline", "--", "-D", "warnings"], env, cwd=project)
        if "wasm32-unknown-unknown" in targets:
            library = project / "wasm.rs"
            library.write_text("pub fn target_word_size() -> usize { std::mem::size_of::<usize>() }\n")
            invoke("wasm-metadata", [rustup, "run", install_label, "rustc", "--target", "wasm32-unknown-unknown", "--crate-type", "lib", "--emit=metadata", library, "-o", project / "wasm.rmeta"], env)
        result["additional_selections"] = []
        for index, name in enumerate(additional_selections or []):
            matches = [r for r in collection_records if r["selection"]["name"] == name]
            if len(matches) != 1 or host not in matches[0]["selection"]["hosts"]:
                raise ValueError("additional selection must identify one record for the tested host")
            record = matches[0]
            selected = record["selection"]
            label = record["pinned_toolchain"]
            extra = ["toolchain", "install", label, "--profile", selected["profile"], "--no-self-update"]
            if selected.get("components"):
                extra.append("--component=" + ",".join(selected["components"]))
            if selected.get("targets"):
                extra.append("--target=" + ",".join(selected["targets"]))
            invoke(f"additional-{index}-install", [rustup, *extra], env)
            version = invoke(f"additional-{index}-version", [rustup, "run", label, "rustc", "--version"], env)
            if not version.stdout.startswith(("rustc " + record["release"] + " ").encode()):
                raise RuntimeError("wrong additional toolchain version")
            invoke(f"additional-{index}-build", [rustup, "run", label, "cargo", "build", "--offline"], env, cwd=project)
            run = invoke(f"additional-{index}-run", [project / "target/debug/offline-proof"], env)
            if run.stdout != b"offline mirror proof ok\n":
                raise RuntimeError("wrong additional toolchain program output")
            result["additional_selections"].append({"name": name, "toolchain": label, "release": record["release"]})
        if additional_selections:
            invoke("restore-original-build", [rustup, "run", install_label, "cargo", "build", "--offline"], env, cwd=project)
            version = invoke("restore-original-version", [rustup, "run", install_label, "rustc", "--version"], env)
            if not version.stdout.startswith(f"rustc {release} ".encode()):
                raise RuntimeError("original toolchain no longer selected correctly")
        result["positive_request_count"] = len(request_log) - client_request_start

        # Each negative control starts with another completely empty Rustup/Cargo home.
        downloaded = [r["path"][len(PREFIX)+1:] for r in request_log[client_request_start:] if r["method"] == "GET" and r["status"] == "200" and r["path"].endswith((".tar.xz", ".tar.zst", ".tar.gz"))]
        if not downloaded:
            raise RuntimeError("no real component download was recorded")
        artifact = downloaded[0]
        stem = next(artifact[:-len(ext)] for ext in (".tar.xz", ".tar.zst", ".tar.gz") if artifact.endswith(ext))
        variants = [n for n, p, h in files if any(n == stem+ext for ext in (".tar.xz", ".tar.zst", ".tar.gz"))]
        result["negative_control_variants"] = variants
        hidden = []
        try:
            for name in variants:
                remote = served / PREFIX.lstrip("/") / name
                missing = remote.with_name(remote.name + ".withheld")
                remote.rename(missing)
                hidden.append((remote, missing))
            _, negative_env, negative_rustup = client("missing")
            failed = invoke("missing-artifact-install", [negative_rustup, *install_args], negative_env, success=False)
            if b"404" not in failed.stderr:
                raise RuntimeError("missing-artifact control failed for a different reason")
        finally:
            for remote, missing in hidden:
                missing.rename(remote)
        changed = []
        try:
            for name in variants:
                remote = served / PREFIX.lstrip("/") / name
                with remote.open("r+b") as f:
                    original = f.read(1)
                    f.seek(0)
                    f.write(bytes([original[0] ^ 1]))
                changed.append((remote, original))
            _, negative_env, negative_rustup = client("corrupt")
            failed = invoke("corrupt-artifact-install", [negative_rustup, *install_args], negative_env, success=False)
            if b"checksum" not in failed.stderr.lower():
                raise RuntimeError("corruption control failed for a different reason")
        finally:
            for remote, original in changed:
                with remote.open("r+b") as f:
                    f.write(original)
        if any(not r["path"].startswith(PREFIX + "/") for r in request_log):
            raise RuntimeError("request escaped the configured Raw prefix")
        result["status"] = "passed"
        result["release"] = release
        result["rustup_version"] = rustup_version
        result["snapshot_inventory_sha256"] = publish_raw.digest(snapshot / "snapshot.toml")
        result["limits"] = ["Not a real Nexus instance or authentication/TLS test", "Only the recorded Linux host, pinned release, selected profile/components/targets and Rustup bootstrap", "No crates.io mirroring, channel update, Windows or macOS execution proof; wasm is metadata typechecking only", "No independent OpenPGP signature verification", "Toolchain requires existing system loader, linker and native libraries"]
    finally:
        if server is not None:
            server.shutdown()
            server.server_close()
        if worker is not None:
            worker.join(timeout=5)
        (evidence / "requests.json").write_text(json.dumps(request_log, indent=2) + "\n")
        (evidence / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
        if "served" in locals():
            # Only our own tempfile-created server tree, never the input snapshot.
            shutil.rmtree(served)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--also-selection", action="append", default=[], help="Additional unique collection record installed in the same home")
    parser.add_argument("--selection", help="Unambiguous collection record name")
    parser.add_argument("--host", help="Explicit native host for multi-host snapshots")
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    prove(args.snapshot, args.evidence.resolve(), args.host, args.selection, args.also_selection)


if __name__ == "__main__":
    main()
