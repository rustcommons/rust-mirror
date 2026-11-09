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


def prove(snapshot, evidence):
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
        release, host, rustup_version = (document[k] for k in ("release", "host", "rustup_version"))
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
        invoke("positive-install", [rustup, "toolchain", "install", release, "--profile", "minimal", "--no-self-update"], env)
        versions = invoke("rustc-version", [rustup, "run", release, "rustc", "--version"], env)
        if not versions.stdout.startswith(f"rustc {release} ".encode()):
            raise RuntimeError("unexpected installed compiler")
        invoke("cargo-version", [rustup, "run", release, "cargo", "--version"], env)
        invoke("rustup-version", [rustup, "--version"], env)
        project = client_root / "hello"
        (project / "src").mkdir(parents=True)
        (project / "Cargo.toml").write_text('[package]\nname = "offline-proof"\nversion = "0.1.0"\nedition = "2021"\n')
        (project / "src/main.rs").write_text('fn main() { println!("offline mirror proof ok"); }\n')
        invoke("cargo-offline-build", [rustup, "run", release, "cargo", "build", "--offline"], env, cwd=project)
        run = invoke("run-built-program", [project / "target/debug/offline-proof"], env)
        if run.stdout != b"offline mirror proof ok\n":
            raise RuntimeError("unexpected program output")
        result["positive_request_count"] = len(request_log)

        # Each negative control starts with another completely empty Rustup/Cargo home.
        artifact = next(n for n, p, h in files if n.startswith("dist/") and n.endswith((".tar.xz", ".tar.zst", ".tar.gz")))
        remote_artifact = served / PREFIX.lstrip("/") / artifact
        missing = remote_artifact.with_name(remote_artifact.name + ".withheld")
        remote_artifact.rename(missing)
        try:
            _, negative_env, negative_rustup = client("missing")
            failed = invoke("missing-artifact-install", [negative_rustup, "toolchain", "install", release, "--profile", "minimal", "--no-self-update"], negative_env, success=False)
            if b"404" not in failed.stderr:
                raise RuntimeError("missing-artifact control failed for a different reason")
        finally:
            missing.rename(remote_artifact)
        with remote_artifact.open("r+b") as f:
            original = f.read(1)
            f.seek(0)
            f.write(bytes([original[0] ^ 1]))
        try:
            _, negative_env, negative_rustup = client("corrupt")
            failed = invoke("corrupt-artifact-install", [negative_rustup, "toolchain", "install", release, "--profile", "minimal", "--no-self-update"], negative_env, success=False)
            if b"checksum" not in failed.stderr.lower():
                raise RuntimeError("corruption control failed for a different reason")
        finally:
            with remote_artifact.open("r+b") as f:
                f.write(original)
        if any(not r["path"].startswith(PREFIX + "/") for r in request_log):
            raise RuntimeError("request escaped the configured Raw prefix")
        result["status"] = "passed"
        result["release"] = release
        result["rustup_version"] = rustup_version
        result["snapshot_inventory_sha256"] = publish_raw.digest(snapshot / "snapshot.toml")
        result["limits"] = ["Not a real Nexus instance or authentication/TLS test", "Only the recorded Linux host, pinned release, minimal profile and Rustup bootstrap", "No crates.io mirroring, additional components, updates, Windows or macOS proof", "No independent OpenPGP signature verification", "Toolchain requires existing system loader, linker and native libraries"]
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
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    prove(args.snapshot, args.evidence.resolve())


if __name__ == "__main__":
    main()
