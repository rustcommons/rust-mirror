#!/usr/bin/env python3
"""Real Cargo HTTPS source-replacement proof with private homes and negative controls.

Uses an existing verified crate mirror containing itoa. Never changes real Cargo
or Rustup homes. TLS keys live only in a temporary directory and are removed.
"""
import argparse
import contextlib
import hashlib
import http.server
import json
import os
from pathlib import Path
import platform
import shutil
import ssl
import subprocess
import tempfile
import threading
import tomllib
from urllib.parse import urlsplit


def command(args, cwd=None, env=None, success=True, timeout=180):
    completed = subprocess.run([str(a) for a in args], cwd=cwd, env=env,
                               capture_output=True, text=True, timeout=timeout)
    if success != (completed.returncode == 0):
        raise RuntimeError(f"unexpected {Path(str(args[0])).name} exit {completed.returncode}: "
                           + completed.stdout[-8000:] + completed.stderr[-8000:])
    return completed


def cargo_paths():
    # Read-only discovery before replacing CARGO_HOME, including on Windows.
    cargo = command(["rustup", "which", "cargo"]).stdout.strip()
    rustc = command(["rustup", "which", "rustc"]).stdout.strip()
    return Path(cargo), Path(rustc)


def environment(home, target, rustc):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("CARGO_", "RUSTFLAGS", "RUSTDOCFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"))}
    env.update(CARGO_HOME=str(home), CARGO_TARGET_DIR=str(target), RUSTC=str(rustc),
               CARGO_HTTP_PROXY="", CARGO_NET_RETRY="0", CARGO_BUILD_JOBS="2", CARGO_TERM_COLOR="never")
    return env


def prove_client(evidence, mirror, base, ca_file, cargo, rustc, fault):
    """Exercise a trusted HTTPS endpoint; fault callback controls only the owned fixture."""
    inventory_bytes = (mirror / "snapshot.toml").read_bytes()
    inventory = tomllib.loads(inventory_bytes.decode())
    assert inventory["schema"] == 4 and inventory["plan"]["catalog"]["crate-output"]["base-url"].rstrip("/") == base
    row = next(v for v in inventory["plan"]["versions"] if v["name"] == "itoa")
    version, checksum = row["version"], row["cksum"]
    project = evidence / "client"
    project.mkdir()
    (project / "src").mkdir()
    (project / "src/main.rs").write_text('fn main() { let mut buffer = itoa::Buffer::new(); assert_eq!(buffer.format(42), "42"); }\n')
    (project / "Cargo.toml").write_text('[package]\nname="mirror-client-proof"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nitoa=' + json.dumps("=" + version) + '\n')
    (project / "Cargo.lock").write_text('version = 4\n\n[[package]]\nname = "mirror-client-proof"\nversion = "0.0.0"\ndependencies = ["itoa"]\n\n[[package]]\nname = "itoa"\nversion = ' + json.dumps(version) + '\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = ' + json.dumps(checksum) + '\n')
    lock_hash = hashlib.sha256((project / "Cargo.lock").read_bytes()).hexdigest()
    config = '[source.crates-io]\nreplace-with="fixture"\n[source.fixture]\nregistry=' + json.dumps("sparse+" + base + "/cargo/index/") + '\n[http]\nssl-verify=true\n'
    cases = []

    def invoke(name, args, home_name, success, trusted=True):
        home = evidence / home_name
        home.mkdir(exist_ok=True)
        (home / "config.toml").write_text(config + ('cainfo=' + json.dumps(str(ca_file)) + '\n' if trusted else ''))
        completed = command([cargo, *args], cwd=project, env=environment(home, evidence / (home_name + "-target"), rustc), success=success)
        (evidence / (name + ".stdout")).write_text(completed.stdout)
        (evidence / (name + ".stderr")).write_text(completed.stderr)
        assert hashlib.sha256((project / "Cargo.lock").read_bytes()).hexdigest() == lock_hash
        cases.append(dict(case=name, expected_success=success, exit=completed.returncode))

    invoke("untrusted-ca", ["fetch", "--locked"], "untrusted-home", False, trusted=False)
    fault("missing")
    invoke("missing-archive-no-upstream-fallback", ["fetch", "--locked"], "missing-home", False)
    fault("corrupt")
    invoke("corrupt-archive-checksum-refusal", ["fetch", "--locked"], "corrupt-home", False)
    fault(None)
    invoke("fresh-offline-refusal", ["fetch", "--locked", "--offline"], "fresh-offline-home", False)
    invoke("https-fetch-locked", ["fetch", "--locked"], "positive-home", True)
    invoke("https-build-locked", ["build", "--locked"], "positive-home", True)
    invoke("populated-offline-run", ["run", "--locked", "--offline"], "positive-home", True)
    report = dict(platform=platform.platform(), cargo=command([cargo, "--version"]).stdout.strip(),
                  rustc=command([rustc, "--version"]).stdout.strip(), plan_id=inventory["plan"]["plan_id"],
                  inventory_sha256=hashlib.sha256(inventory_bytes).hexdigest(), lock_sha256=lock_hash,
                  root=dict(name="itoa", version=version, cksum=checksum), tls_verification=True, cases=cases,
                  scope="One real itoa fetch/build/run; not all corpus crates built or foreign targets executed")
    (evidence / "client-proof.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


@contextlib.contextmanager
def certificates(dns_names=None):
    openssl = shutil.which("openssl")
    if platform.system() == "Darwin":
        # Hosted macOS keeps OpenSSL 3 keg-only; do not substitute system LibreSSL.
        for candidate in (Path("/opt/homebrew/opt/openssl@3/bin/openssl"), Path("/usr/local/opt/openssl@3/bin/openssl")):
            if candidate.is_file():
                openssl = str(candidate)
                break
    if openssl is None:
        raise RuntimeError("OpenSSL fixture prerequisite missing; runtime rust-mirror does not use it")
    with tempfile.TemporaryDirectory(prefix="rust-mirror-crates-tls-") as directory:
        pki = Path(directory)
        command([openssl, "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-sha256", "-days", "1", "-subj", "/CN=rust-mirror-disposable-ca", "-addext", "basicConstraints=critical,CA:TRUE", "-addext", "keyUsage=critical,keyCertSign,cRLSign", "-keyout", pki / "ca.key", "-out", pki / "ca.pem"])
        (pki / "index.txt").write_text("")
        (pki / "crlnumber").write_text("01\n")
        configuration = "[ca]\ndefault_ca=fixture\n[fixture]\ndefault_md=sha256\ndefault_crl_days=1\n"
        for key, name in (("database", "index.txt"), ("crlnumber", "crlnumber"), ("private_key", "ca.key"), ("certificate", "ca.pem")):
            configuration += key + "=" + (pki / name).as_posix() + "\n"
        (pki / "ca.cnf").write_text(configuration)
        command([openssl, "ca", "-gencrl", "-config", pki / "ca.cnf", "-out", pki / "fixture-crl.pem"])
        command([openssl, "crl", "-in", pki / "fixture-crl.pem", "-outform", "DER", "-out", pki / "fixture.crl"])
        crl_requests = []
        (pki / "crl-requests.json").write_text("[]\n")

        class Crl(http.server.BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def do_GET(self):
                # Serve only the signed public CRL, never the private PKI directory.
                if self.path != "/fixture.crl":
                    self.send_error(404)
                    return
                crl_requests.append(self.path)
                (pki / "crl-requests.json").write_text(json.dumps(crl_requests) + "\n")
                data = (pki / "fixture.crl").read_bytes()
                self.send_response(200)
                self.send_header("Content-Type", "application/pkix-crl")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

        crl_server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Crl)
        crl_server.daemon_threads = True
        crl_thread = threading.Thread(target=crl_server.serve_forever, daemon=True)
        crl_thread.start()
        try:
            command([openssl, "req", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=localhost", "-keyout", pki / "server.key", "-out", pki / "server.csr"])
            names = dns_names or ["localhost"]
            san = ",".join("DNS:" + name for name in names) + ",IP:127.0.0.1"
            (pki / "server.ext").write_text("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=" + san + "\ncrlDistributionPoints=URI:http://127.0.0.1:" + str(crl_server.server_port) + "/fixture.crl\n")
            command([openssl, "x509", "-req", "-in", pki / "server.csr", "-CA", pki / "ca.pem", "-CAkey", pki / "ca.key", "-CAcreateserial", "-out", pki / "server.pem", "-days", "1", "-sha256", "-extfile", pki / "server.ext"])
            command([openssl, "verify", "-CAfile", pki / "ca.pem", "-crl_check", "-CRLfile", pki / "fixture-crl.pem", pki / "server.pem"])
            yield pki
        finally:
            crl_server.shutdown()
            crl_server.server_close()
            crl_thread.join(timeout=10)
            (pki / "crl-requests.json").write_text(json.dumps(crl_requests) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mirror", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--cargo", type=Path)
    parser.add_argument("--rustc", type=Path)
    args = parser.parse_args()
    args.evidence = args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=False)
    mirror = args.mirror.resolve()
    inventory = tomllib.loads((mirror / "snapshot.toml").read_text())
    base = inventory["plan"]["catalog"]["crate-output"]["base-url"].rstrip("/")
    url = urlsplit(base)
    if url.scheme != "https" or url.hostname not in ("localhost", "127.0.0.1") or not url.port or url.username or url.password or url.query or url.fragment:
        parser.error("mirror must be planned for an explicit localhost/127.0.0.1 HTTPS port")
    cargo, rustc = (args.cargo, args.rustc) if args.cargo and args.rustc else cargo_paths()
    requests = []
    fault = {"mode": None}

    class Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *a, **kw):
            super().__init__(*a, directory=str(mirror), **kw)

        def log_message(self, *a):
            pass

        def do_GET(self):
            requests.append(dict(path=self.path, fault=fault["mode"]))
            prefix = url.path + "/"
            if not self.path.startswith(prefix) or "?" in self.path:
                self.send_error(404)
                return
            if self.path.endswith(".crate") and fault["mode"]:
                if fault["mode"] == "missing":
                    self.send_error(404)
                else:
                    data = b"checksum-negative-control"
                    self.send_response(200)
                    self.send_header("Content-Length", str(len(data)))
                    self.end_headers()
                    self.wfile.write(data)
                return
            self.path = self.path[len(url.path):]
            super().do_GET()

        def list_directory(self, path):
            self.send_error(404)
            return None

    with certificates() as pki:
        ca_file = args.evidence / "test-ca.pem"
        shutil.copyfile(pki / "ca.pem", ca_file)
        server = http.server.ThreadingHTTPServer(("127.0.0.1", url.port), Handler)
        server.daemon_threads = True
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version = ssl.TLSVersion.TLSv1_2
        context.load_cert_chain(pki / "server.pem", pki / "server.key")
        server.socket = context.wrap_socket(server.socket, server_side=True)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            result = prove_client(args.evidence, mirror, base, ca_file, cargo, rustc, lambda mode: fault.update(mode=mode))
            for mode in (None, "missing", "corrupt"):
                assert any(r["path"].endswith(".crate") and r["fault"] == mode for r in requests), f"archive control was not exercised: {mode}"
            assert "checksum" in (args.evidence / "corrupt-archive-checksum-refusal.stderr").read_text().lower()
            print(json.dumps(result), flush=True)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=10)
            shutil.copyfile(pki / "crl-requests.json", args.evidence / "revocation-requests.json")
            (args.evidence / "requests.json").write_text(json.dumps(requests, indent=2) + "\n")


if __name__ == "__main__":
    main()
