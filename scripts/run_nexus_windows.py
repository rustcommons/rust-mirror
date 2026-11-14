#!/usr/bin/env python3
"""Owned native Windows Nexus OSS proof; never accepts agreements or external targets."""
import argparse
import contextlib
import hashlib
import http.client
import http.server
import json
import os
from pathlib import Path
import platform
import secrets
import shutil
import socket
import ssl
import stat
import subprocess
import tempfile
import threading
import time
import urllib.request
import zipfile
from prove_nexus import credentials
from prove_crates_https import certificates, cargo_paths, command, environment

BUNDLE = "https://download.sonatype.com/nexus/3/nexus-3.76.1-01-win64.zip"
CHECKSUM = "8522ed370b2689667d4e6bee9414d27c10f263a7e0167af957beca1c6cdb8991"
PROJECT = "abc20a380c8c2d9d76c1976222725d3211cef809"
LOCK_SHA = "3343803df2472c7602d40214bfc42dff094a28ff0a25a7f02cb72ffffea18b9e"


def download(url, path, maximum):
    request = urllib.request.Request(url, headers={"User-Agent": "rust-mirror-qualification (https://github.com/rustcommons/rust-mirror)"})
    with urllib.request.urlopen(request, timeout=120) as response, path.open("xb") as output:
        count = 0
        while block := response.read(1024 * 1024):
            count += len(block)
            if count > maximum:
                raise ValueError("download exceeds qualification bound")
            output.write(block)


def extract_bundle(archive, root):
    with zipfile.ZipFile(archive) as source:
        total = 0
        for member in source.infolist():
            path = Path(member.filename)
            if path.is_absolute() or ".." in path.parts or ":" in member.filename or "\\" in member.filename:
                raise ValueError("unsafe bundle path")
            if stat.S_ISLNK(member.external_attr >> 16):
                raise ValueError("bundle symlink refused")
            total += member.file_size
            if total > 1024 * 1024 * 1024:
                raise ValueError("bundle extraction exceeds bound")
        source.extractall(root)


def unused_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def cleanup(root):
    """Delete only our marked instance; a reused PID must match the owned executable."""
    marker = root / "ownership.json"
    if not marker.exists():
        if root.exists():
            raise ValueError("cleanup refuses unmarked directory")
        return {"instance_removed": True, "private_keys_destroyed": True}
    state = json.loads(marker.read_text())
    if state.get("kind") != "rust-mirror-native-nexus" or Path(state["root"]).resolve() != root.resolve():
        raise ValueError("cleanup ownership mismatch")
    pid = state.get("pid")
    if pid:
        query = "$p=Get-CimInstance Win32_Process -Filter 'ProcessId = " + str(int(pid)) + "'; if($p){$p.ExecutablePath}"
        found = subprocess.run(["powershell", "-NoProfile", "-Command", query], capture_output=True, text=True, timeout=30)
        if found.returncode:
            raise RuntimeError("cannot verify owned process for cleanup")
        path = found.stdout.strip()
        if path:
            if Path(path).resolve() != (root / "nexus-3.76.1-01/bin/nexus.exe").resolve():
                raise RuntimeError("cleanup refuses a reused PID")
            subprocess.run(["taskkill", "/PID", str(pid), "/T", "/F"], capture_output=True, timeout=60, check=True)
    # Java/Defender can briefly retain handles after process exit.
    for attempt in range(12):
        try:
            shutil.rmtree(root)
            break
        except OSError:
            if attempt == 11:
                raise
            threading.Event().wait(1)
    return {"instance_removed": not root.exists(), "private_keys_destroyed": not root.exists()}


@contextlib.contextmanager
def nexus(root, evidence):
    root.mkdir(parents=True, exist_ok=False)
    state = {"kind": "rust-mirror-native-nexus", "root": str(root), "nonce": secrets.token_hex(16)}
    marker = root / "ownership.json"
    marker.write_text(json.dumps(state))
    process = None
    try:
        sidecar = root / "bundle.sha256"
        download(BUNDLE + ".sha256", sidecar, 1024)
        if sidecar.read_text().strip() != CHECKSUM:
            raise ValueError("published bundle checksum changed; review required")
        archive = root / "bundle.zip"
        download(BUNDLE, archive, 512 * 1024 * 1024)
        if hashlib.sha256(archive.read_bytes()).hexdigest() != CHECKSUM:
            raise ValueError("official bundle checksum mismatch")
        extract_bundle(archive, root)
        home = root / "nexus-3.76.1-01"
        data = root / "sonatype-work/nexus3"
        (data / "etc").mkdir(parents=True, exist_ok=True)
        port = unused_port()
        (data / "etc/nexus.properties").write_text(f"application-host=127.0.0.1\napplication-port={port}\n")
        options = home / "bin/nexus.vmoptions"
        options.write_text(options.read_text().replace("-Xms2703m", "-Xms256m").replace("-Xmx2703m", "-Xmx1024m") + "\n-XX:MaxDirectMemorySize=256m\n")
        java = home / "jre/bin/java.exe"
        version = command([java, "-version"])
        env = {k: v for k, v in os.environ.items() if k.upper() not in ("JAVA_HOME", "JRE_HOME", "APP_JAVA_HOME", "INSTALL4J_JAVA_HOME_OVERRIDE", "JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS", "JDK_JAVA_OPTIONS")}
        # Force the bundled JRE; no machine-wide installation or service registration.
        env["INSTALL4J_JAVA_HOME_OVERRIDE"] = str(home / "jre")
        with (root / "private-startup.log").open("wb") as log:
            process = subprocess.Popen([str(home / "bin/nexus.exe"), "/run"], cwd=home, env=env, stdout=log, stderr=subprocess.STDOUT)
        state["pid"] = process.pid
        marker.write_text(json.dumps(state))
        deadline = time.monotonic() + 420
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError("official win64 Nexus could not run; stop for owner review")
            try:
                conn = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
                conn.request("GET", "/service/rest/v1/status")
                response = conn.getresponse()
                server = response.getheader("Server", "")
                response.read()
                conn.close()
                if response.status == 200 and (data / "admin.password").is_file():
                    assert "3.76.1" in server, server
                    yield port, (data / "admin.password").read_text().strip(), {"bundle_url": BUNDLE, "checksum_url": BUNDLE + ".sha256", "sha256": CHECKSUM, "server": server, "bundled_java": (version.stdout + version.stderr).strip(), "native_windows": True}
                    return
            except (OSError, http.client.HTTPException):
                pass
            threading.Event().wait(1)
        raise RuntimeError("official win64 Nexus readiness timed out; stop for owner review")
    finally:
        outcome = cleanup(root)
        if process:
            process.wait(timeout=30)
        (evidence / "cleanup.json").write_text(json.dumps(outcome, indent=2) + "\n")


@contextlib.contextmanager
def tls_proxy(port, pki, requests):
    class Proxy(http.server.BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def forward(self):
            if not self.path.startswith(("/repository/", "/service/rest/v1/")) or "?" in self.path or "#" in self.path:
                self.send_error(400)
                return
            conn = http.client.HTTPConnection("127.0.0.1", port, timeout=120)
            try:
                size = int(self.headers.get("Content-Length", "0"))
                if size < 0 or size > 512 * 1024 * 1024:
                    raise ValueError("request body bound")
                conn.putrequest(self.command, self.path)
                for k, v in self.headers.items():
                    if k.lower() not in ("host", "connection", "transfer-encoding", "expect"):
                        conn.putheader(k, v)
                conn.endheaders()
                while size:
                    block = self.rfile.read(min(size, 65536))
                    if not block:
                        raise ValueError("short request body")
                    conn.send(block)
                    size -= len(block)
                response = conn.getresponse()
                requests.append({"method": self.command, "path": self.path, "status": response.status})
                self.send_response_only(response.status)
                for k, v in response.getheaders():
                    if k.lower() not in ("connection", "transfer-encoding"):
                        self.send_header(k, v)
                self.end_headers()
                if self.command != "HEAD":
                    shutil.copyfileobj(response, self.wfile, 65536)
            finally:
                conn.close()

        do_GET = do_HEAD = do_POST = do_PUT = do_DELETE = forward

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Proxy)
    server.daemon_threads = True
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls.minimum_version = ssl.TLSVersion.TLSv1_2
    tls.load_cert_chain(pki / "server.pem", pki / "server.key")
    server.socket = tls.wrap_socket(server.socket, server_side=True)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server.server_port
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--cleanup", action="store_true")
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--probe-only", action="store_true")
    args = parser.parse_args()
    if platform.system() != "Windows":
        parser.error("native Windows only; no external Nexus endpoint accepted")
    root = args.root.resolve()
    if args.cleanup:
        print(json.dumps(cleanup(root)))
        return
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    result = {"status": "starting", "cases": [], "source": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()}
    requests = []
    try:
        with nexus(root, evidence) as (port, password, identity):
            result["instance"] = identity
            private = root / "private-tmp"
            private.mkdir()
            tempfile.tempdir = str(private)
            with certificates() as pki, tls_proxy(port, pki, requests) as tls_port:
                trust = ssl.create_default_context(cafile=str(pki / "ca.pem"))
                admin = credentials("admin", password)
                def request(method, path, data=None, auth=admin, text=False):
                    conn = http.client.HTTPSConnection("127.0.0.1", tls_port, context=trust, timeout=120)
                    body = data.encode() if text else json.dumps(data).encode() if data is not None else None
                    try:
                        headers = {"Content-Type": "text/plain" if text else "application/json"}
                        if auth:
                            headers["Authorization"] = auth
                        conn.request(method, path, body=body, headers=headers)
                        r = conn.getresponse()
                        b = r.read(4 * 1024 * 1024 + 1)
                        if len(b) > 4 * 1024 * 1024:
                            raise ValueError("response bound")
                        if r.status == 403 and (b"license agreement" in b.lower() or b"eula" in b.lower()):
                            raise RuntimeError("EULA acceptance required; stop, no acceptance attempted")
                        return r.status, b
                    finally:
                        conn.close()
                repo = "rust-mirror-win-" + secrets.token_hex(6)
                status, _ = request("POST", "/service/rest/v1/repositories/raw/hosted", {"name": repo, "online": True, "storage": {"blobStoreName": "default", "strictContentTypeValidation": True, "writePolicy": "ALLOW_ONCE"}, "cleanup": {"policyNames": []}, "raw": {"contentDisposition": "ATTACHMENT"}})
                assert status == 201, ("create owned repo", status)
                status, _ = request("GET", "/repository/" + repo + "/onboarding-check")
                assert status == 404, ("content gate", status)
                result["cases"].append({"case": "native_oss_start_and_repository_without_eula_acceptance", "passed": True})
                if not args.probe_only:
                    raise RuntimeError("client qualification not implemented yet")
                result["status"] = "passed"
    finally:
        (evidence / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
        (evidence / "requests.json").write_text(json.dumps(requests, indent=2) + "\n")


if __name__ == "__main__":
    main()
