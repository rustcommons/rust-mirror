#!/usr/bin/env python3
"""Kill/cancel the actual CLI through an owned TLS-inspecting fixture; resume exact bytes.

Synthetic opaque payload, not a Cargo-client/archive-format proof. Fixed official
origins still flow through NetworkOptions and verified TLS; no upstream requests.
"""
import argparse
import hashlib
import http.server
import json
from pathlib import Path
import platform
import queue
import shutil
import ssl
import subprocess
import threading
import time
from urllib.parse import urlsplit
from prove_crates_https import certificates


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    payload = bytes(range(256)) * 4096
    digest = hashlib.sha256(payload).hexdigest()
    metadata = json.dumps(dict(name="x", vers="1.0.0", deps=[], features={}, cksum=digest, yanked=False)).encode() + b"\n"
    requests = []
    rate_limit = {"pending": True}
    active_children = []
    report = {"platform": platform.platform(), "payload_bytes": len(payload), "payload_sha256": digest, "scope": "Synthetic TLS CONNECT fixture; actual CLI kill/cancel/resume, not an archive-format or Nexus proof"}
    with certificates(["index.crates.io", "static.crates.io"]) as pki:
        ca_file = evidence / "test-ca.pem"
        shutil.copyfile(pki / "ca.pem", ca_file)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(pki / "server.pem", pki / "server.key")

        class Origin(http.server.BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def do_GET(self):
                requests.append(dict(observed_path=self.path))
                if self.path.startswith("https://"):
                    parsed = urlsplit(self.path)
                    if parsed.netloc not in ("index.crates.io", "static.crates.io") or parsed.query or parsed.fragment:
                        self.send_error(403)
                        return
                    self.path = parsed.path
                if self.path == "/1/x":
                    self.send_response(200)
                    self.send_header("Content-Length", str(len(metadata)))
                    self.end_headers()
                    self.wfile.write(metadata)
                    return
                if self.path != "/crates/x/x-1.0.0.crate":
                    self.send_error(404)
                    return
                supplied = self.headers.get("Range")
                offset = int(supplied.removeprefix("bytes=").removesuffix("-")) if supplied else 0
                requests.append(dict(path=self.path, offset=offset))
                if rate_limit["pending"]:
                    rate_limit["pending"] = False
                    self.send_response(429)
                    self.send_header("Retry-After", "1")
                    self.send_header("Content-Length", "0")
                    self.end_headers()
                    return
                self.send_response(206 if offset else 200)
                self.send_header("Content-Length", str(len(payload) - offset))
                if offset:
                    self.send_header("Content-Range", f"bytes {offset}-{len(payload)-1}/{len(payload)}")
                self.end_headers()
                # Heartbeats must continue while the read is blocked, before more bytes arrive.
                time.sleep(1.4)
                try:
                    for start in range(offset, len(payload), 65536):
                        self.wfile.write(payload[start:start+65536])
                        self.wfile.flush()
                        time.sleep(0.04)
                except (OSError, ssl.SSLError):
                    pass  # The owned child is deliberately killed/cancelled.

        class Proxy(http.server.BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def do_CONNECT(self):
                if self.path not in ("index.crates.io:443", "static.crates.io:443"):
                    self.send_error(403)
                    return
                self.send_response(200)
                self.end_headers()
                try:
                    with context.wrap_socket(self.connection, server_side=True) as connection:
                        Origin(connection, self.client_address, self.server)
                except (OSError, ssl.SSLError):
                    pass
                self.close_connection = True

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Proxy)
        server.daemon_threads = True
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        network = ["--proxy", f"http://127.0.0.1:{server.server_port}", "--extra-ca-file", str(ca_file)]
        catalog = evidence / "catalog.toml"
        catalog.write_text('[crates]\nx="=1.0.0"\n[crate-policy]\nhosts=["x86_64-unknown-linux-gnu","x86_64-pc-windows-msvc"]\ntargets=["x86_64-unknown-linux-gnu","x86_64-pc-windows-msvc"]\n[crate-output]\nbase-url="https://mirror.example/fixture"\n[crate-network]\ncontact="https://github.com/rustcommons/rust-mirror/issues"\n')

        def run(name, command, stop=None, expected=True):
            process = subprocess.Popen([str(binary), "--events", "jsonl", *( ["--cancel-stdin"] if stop == "cancel" else [] ), "crates", *map(str, command), *network], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            active_children.append(process)
            lines = queue.Queue()
            def consume():
                try:
                    for line in process.stdout:
                        lines.put(line)
                finally:
                    lines.put(None)
            reader = threading.Thread(target=consume, daemon=True)
            reader.start()
            records = []
            sent = False
            deadline = time.monotonic() + 45
            try:
                while time.monotonic() < deadline:
                    line = lines.get(timeout=max(0.1, deadline-time.monotonic()))
                    if line is None:
                        break
                    event = json.loads(line)
                    records.append(event)
                    if stop and not sent and event["type"] == "bytes" and event["path"].endswith(".crate") and event["count"] >= 65536:
                        sent = True
                        if stop == "kill":
                            process.kill()
                        else:
                            process.stdin.write('{"type":"cancel"}\n')
                            process.stdin.flush()
                code = process.wait(timeout=10)
                stderr = process.stderr.read()
                if stop:
                    assert sent, "stop trigger not exercised"
                if stop != "kill":
                    assert records[-1]["type"] == "result" and records[-1]["ok"] == expected, records[-1]
                    assert sum(r["type"] == "result" for r in records) == 1
                    if stop == "cancel":
                        assert records[-1]["error"]["kind"] == "cancelled"
                else:
                    assert code != 0 and not any(r["type"] == "result" and r["ok"] for r in records)
                (evidence / (name + ".jsonl")).write_text("".join(json.dumps(e) + "\n" for e in records))
                (evidence / (name + ".stderr")).write_text(stderr)
                return records
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=10)
                reader.join(timeout=5)
                for stream in (process.stdin, process.stdout, process.stderr):
                    if stream:
                        stream.close()

        try:
            preview = run("plan", ["plan", "--config", catalog, "--output", evidence / "plan.json"])
            plan_id = preview[-1]["report"]["plan_id"]
            for mode in ("kill", "cancel"):
                output = evidence / (mode + "-run")
                command = ["fetch", "--plan", evidence / "plan.json", "--output", output, "--approve-plan", plan_id]
                stopped = run(mode, command, stop=mode, expected=False)
                assert not (output / "mirror").exists()
                partials = list((output / ".download/files").glob("*.part"))
                assert len(partials) == 1 and 0 < partials[0].stat().st_size < len(payload)
                resumed = run(mode + "-resume", [*command, "--resume"])
                archive = output / "mirror/cargo/crates/x/x-1.0.0.crate"
                assert hashlib.sha256(archive.read_bytes()).hexdigest() == digest
                assert any(r["type"] == "heartbeat" and (r.get("path") or "").endswith(".crate") for r in stopped + resumed)
                report[mode] = dict(partial_preserved=True, resumed=True, final=resumed[-1]["report"])
            assert any(r.get("offset", 0) > 0 for r in requests)
            first = [json.loads(line) for line in (evidence / "kill.jsonl").read_text().splitlines()]
            assert any(r["type"] == "retry" and r["kind"] == "rate_limited" and r["wait_seconds"] == 1 for r in first)
            report["ranges"] = requests
            report["retry_notice_and_blocked_read_heartbeats"] = True
            (evidence / "resume-proof.json").write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(report), flush=True)
        finally:
            for process in active_children:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=10)
            server.shutdown()
            server.server_close()
            worker.join(timeout=10)
            (evidence / "requests.json").write_text(json.dumps(requests, indent=2) + "\n")


if __name__ == "__main__":
    main()
