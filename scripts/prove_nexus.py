#!/usr/bin/env python3
"""Destructive test of a NEW disposable Nexus instance, inside its network namespace.

Use run_nexus_proof.py. This is NOT a production configuration/migration utility.
The only API target is namespace-local 127.0.0.1. No external interface is allowed.
"""
import argparse
import base64
import http.client
import http.server
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import ssl
import subprocess
import tempfile
import threading
import time
import tomllib
from urllib.request import urlopen
import publish_raw
from urllib.parse import urlsplit

REPO = "rust-mirror-proof"
SCOPE_REPO = "rust-mirror-scope-control"


def credentials(user, password):
    return "Basic " + base64.b64encode(f"{user}:{password}".encode()).decode()


def native_selection(document):
    if document["schema"] == 1:
        return document["release"], document["host"], document["release"]
    records = document.get("toolchains", [document])
    record = next((r for r in sorted(records, key=lambda r:r["date"], reverse=True)
                   if "x86_64-unknown-linux-gnu" in r["selection"]["hosts"]), None)
    if record is None: raise ValueError("Nexus native proof needs a GNU x86-64 selection")
    return record["release"], "x86_64-unknown-linux-gnu", record["pinned_toolchain"]

def prove(snapshot, evidence, password_file, nexus_port, allow_raw_mime_types=False, next_snapshot=None, publisher_cli=None, cargo=None, rustc=None):
    evidence.mkdir(parents=True, exist_ok=False, mode=0o700)
    result = {"status": "running", "cases": [], "commands": [], "scope": "Disposable actual Nexus Raw hosted, with a temporary TLS reverse proxy"}
    requests = []
    fault = {"mode": None}
    server = None
    worker = None
    try:
        subprocess.run(["ip", "link", "set", "lo", "up"], check=True)
        routes = json.loads(subprocess.check_output(["ip", "-json", "route"]))
        interfaces = json.loads(subprocess.check_output(["ip", "-json", "address"]))
        if routes or {i["ifname"] for i in interfaces} != {"lo"}:
            raise RuntimeError("refusing a namespace with external interfaces or routes")
        result["network"] = {"routes": routes, "interfaces": interfaces}
        try:
            with socket.create_connection(("1.1.1.1", 443), timeout=1):
                raise RuntimeError("external network unexpectedly reachable")
        except OSError:
            result["external_connection_refused"] = True
        candidate = tomllib.loads((snapshot / "snapshot.toml").read_text())
        crate_mode = candidate.get("schema") == 4
        crate_port = 0
        if crate_mode:
            if publisher_cli is None or cargo is None or rustc is None:
                raise ValueError("crate proof needs native publisher and explicit Cargo/rustc paths")
            document = candidate
            files = [(f["path"], snapshot / f["path"], f["sha256"]) for f in document["files"]]
            files.append(("snapshot.toml", snapshot / "snapshot.toml", publish_raw.digest(snapshot / "snapshot.toml")))
            crate_base = document["plan"]["catalog"]["crate-output"]["base-url"].rstrip("/")
            parsed = urlsplit(crate_base)
            if parsed.scheme != "https" or parsed.hostname != "127.0.0.1" or not parsed.port or parsed.username or parsed.password or parsed.query or parsed.fragment or parsed.path != f"/repository/{REPO}/snapshots/proof-001":
                raise ValueError("crate generation must target only the owned namespace-local proof prefix")
            crate_port = parsed.port
        else:
            document, files = publish_raw.local_files(snapshot)
        result["snapshot_schema"] = document["schema"]
        if next_snapshot is not None:
            next_document, _ = publish_raw.local_files(next_snapshot)
            if next_document["schema"] != 3 or document["schema"] not in (2, 3):
                raise ValueError("lifecycle proof requires selected/collection generations")
        result["inventory_sha256"] = publish_raw.digest(snapshot / "snapshot.toml")
        def publish_snapshot(source, destination, ca_file):
            if publisher_cli is None: return publish_raw.publish(source,destination,ca_file=ca_file)
            command=[str(publisher_cli),"publish","--source",str(source),"--base-url",destination,"--ca-file",str(ca_file)]
            completed=subprocess.run(command,capture_output=True,timeout=900)
            if completed.returncode: raise ValueError(completed.stderr.decode("utf-8",errors="replace"))
            return tomllib.loads(completed.stdout.decode("utf-8"))
        admin_password = password_file.read_text().strip()
        administrator = credentials("admin", admin_password)

        def case(name, **data):
            result["cases"].append({"case": name, "passed": True, **data})

        def api(method, path, data=None, text=False):
            connection = http.client.HTTPConnection("127.0.0.1", nexus_port, timeout=30)
            try:
                body = data.encode() if text else json.dumps(data).encode() if data is not None else None
                headers = {"Authorization": administrator, "Content-Type": "text/plain" if text else "application/json"}
                connection.request(method, "/service/rest/v1/" + path, body=body, headers=headers)
                response = connection.getresponse()
                payload = response.read(4 * 1024 * 1024 + 1)
                if response.status not in (200, 201, 204):
                    # Do not log potentially credential-bearing request/response bodies.
                    raise RuntimeError(f"Nexus API {method} {path} returned HTTP {response.status}")
                if len(payload) > 4 * 1024 * 1024:
                    raise RuntimeError("Nexus API response exceeds budget")
                return json.loads(payload) if payload else None
            finally:
                connection.close()

        deadline = time.monotonic() + 300
        while time.monotonic() < deadline:
            try:
                connection = http.client.HTTPConnection("127.0.0.1", nexus_port, timeout=2)
                connection.request("GET", "/service/rest/v1/status")
                response = connection.getresponse()
                version = response.getheader("Server", "")
                response.read()
                connection.close()
                if response.status == 200:
                    result["nexus_server"] = version
                    break
            except OSError:
                pass
            time.sleep(1)
        else:
            raise TimeoutError("Nexus REST readiness timed out")
        case("actual_nexus_rest_ready", server=result["nexus_server"])
        existing = api("GET", "repositories")
        if any(r["name"] in (REPO, SCOPE_REPO) for r in existing):
            raise RuntimeError("refusing to alter an existing proof repository")
        for name in (REPO, SCOPE_REPO):
            api("POST", "repositories/raw/hosted", {"name": name, "online": True, "storage": {"blobStoreName": "default", "strictContentTypeValidation": True, "writePolicy": "ALLOW_ONCE"}, "cleanup": {"policyNames": []}, "raw": {"contentDisposition": "ATTACHMENT"}})
        configuration = api("GET", "repositories/raw/hosted/" + REPO)
        if configuration["storage"]["writePolicy"].lower() != "allow_once":
            raise RuntimeError("Nexus did not retain no-redeploy policy")
        case("raw_hosted_no_redeploy", policy=configuration["storage"]["writePolicy"])
        gate = http.client.HTTPConnection("127.0.0.1", nexus_port, timeout=10)
        gate.request("GET", f"/repository/{REPO}/onboarding-check", headers={"Authorization": administrator})
        response = gate.getresponse()
        gate_body = response.read(8192).decode("utf-8", errors="replace")
        gate.close()
        if response.status == 403 and "End User License Agreement" in gate_body:
            result["status"] = "held"
            result["blocker"] = "Server requires operator EULA acceptance; this proof never accepts it."
            raise RuntimeError(result["blocker"])

        publisher_role = "rust-mirror-publish"
        reader_role = "rust-mirror-read"
        for role, actions in [(publisher_role, ["browse", "read", "add", "edit"]), (reader_role, ["browse", "read"])]:
            api("POST", "security/roles", {"id": role, "name": role, "description": "Disposable rust-mirror integration proof", "privileges": [f"nx-repository-view-raw-{REPO}-{a}" for a in actions], "roles": []})
        publisher = "rust-mirror-publisher"
        publisher_password = secrets.token_urlsafe(32)
        api("POST", "security/users", {"userId": publisher, "firstName": "Mirror", "lastName": "Proof", "emailAddress": "mirror@example.invalid", "password": publisher_password, "status": "active", "roles": [publisher_role]})
        reader = "rust-mirror-reader"
        api("POST", "security/users", {"userId": reader, "firstName": "Anonymous", "lastName": "Reader", "emailAddress": "reader@example.invalid", "password": secrets.token_urlsafe(32), "status": "active", "roles": [reader_role]})
        api("PUT", "security/anonymous", {"enabled": True, "userId": reader, "realmName": "NexusAuthorizingRealm"})
        replacement = secrets.token_urlsafe(32)
        api("PUT", "security/users/admin/change-password", replacement, text=True)
        administrator = credentials("admin", replacement)
        case("scoped_publisher_and_read_only_anonymous_roles")
        result["configured_users"] = [{k: u.get(k) for k in ("userId", "source", "status", "roles")} for u in api("GET", "security/users") if u.get("userId") in (publisher, reader)]
        result["configured_publisher_role"] = {k: v for k, v in api("GET", "security/roles/" + publisher_role).items() if k in ("id", "source", "privileges", "roles")}


        with tempfile.TemporaryDirectory(prefix="rust-mirror-test-tls-") as tls_path:
            tls = Path(tls_path)
            subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", str(tls / "ca.key"), "-out", str(tls / "ca.pem"), "-days", "1", "-subj", "/CN=rust-mirror-disposable-ca", "-addext", "basicConstraints=critical,CA:TRUE", "-addext", "keyUsage=critical,keyCertSign,cRLSign"], capture_output=True, check=True)
            subprocess.run(["openssl", "req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", str(tls / "server.key"), "-out", str(tls / "server.csr"), "-subj", "/CN=127.0.0.1"], capture_output=True, check=True)
            (tls / "server.ext").write_text("subjectAltName=IP:127.0.0.1\nextendedKeyUsage=serverAuth\nbasicConstraints=critical,CA:FALSE\n")
            subprocess.run(["openssl", "x509", "-req", "-in", str(tls / "server.csr"), "-CA", str(tls / "ca.pem"), "-CAkey", str(tls / "ca.key"), "-CAcreateserial", "-out", str(tls / "server.pem"), "-days", "1", "-extfile", str(tls / "server.ext")], capture_output=True, check=True)
            ca_file = evidence / "test-ca.pem"
            shutil.copyfile(tls / "ca.pem", ca_file)
            context = ssl.create_default_context(cafile=ca_file)

            class Proxy(http.server.BaseHTTPRequestHandler):
                def log_request(self, code="-", size="-"):
                    requests.append({"method": self.command, "path": self.path, "status": str(code), "authenticated": bool(self.headers.get("Authorization"))})

                def log_message(self, *args):
                    pass

                def forward(self):
                    if self.command == "GET" and fault["mode"] and self.path.endswith((".tar.gz", ".tar.xz", ".tar.zst", ".crate")):
                        if fault["mode"] == "missing": self.send_error(404); return
                        data = b"deliberately corrupt proxy control"
                        self.send_response(200); self.send_header("Content-Length", str(len(data))); self.end_headers(); self.wfile.write(data); return

                    if not self.path.startswith("/repository/") or "?" in self.path:
                        self.send_error(400)
                        return
                    connection = http.client.HTTPConnection("127.0.0.1", nexus_port, timeout=120)
                    try:
                        headers = {k: v for k, v in self.headers.items() if k.lower() not in ("host", "connection", "transfer-encoding")}
                        connection.putrequest(self.command, self.path)
                        for key, value in headers.items():
                            connection.putheader(key, value)
                        connection.endheaders()
                        remaining = int(self.headers.get("Content-Length", "0"))
                        if remaining > 512 * 1024 * 1024:
                            raise ValueError("proxy input exceeds budget")
                        while remaining:
                            chunk = self.rfile.read(min(remaining, 1024 * 1024))
                            if not chunk:
                                raise RuntimeError("incomplete request")
                            connection.send(chunk)
                            remaining -= len(chunk)
                        response = connection.getresponse()
                        self.send_response(response.status)
                        for key, value in response.getheaders():
                            if key.lower() not in ("transfer-encoding", "connection", "server", "date"):
                                self.send_header(key, value)
                        self.end_headers()
                        if self.command != "HEAD":
                            shutil.copyfileobj(response, self.wfile, 1024 * 1024)
                    finally:
                        connection.close()

                do_GET = forward
                do_HEAD = forward
                do_PUT = forward

            server = http.server.ThreadingHTTPServer(("127.0.0.1", crate_port), Proxy)
            server.daemon_threads = True
            server_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            server_context.minimum_version = ssl.TLSVersion.TLSv1_2
            server_context.load_cert_chain(tls / "server.pem", tls / "server.key")
            server.socket = server_context.wrap_socket(server.socket, server_side=True)
            worker = threading.Thread(target=server.serve_forever, daemon=True)
            worker.start()
            base = f"https://127.0.0.1:{server.server_port}/repository/{REPO}/snapshots/proof-001"
            if crate_mode and base != crate_base: raise ValueError("crate proof listener does not match immutable dl URL")

            def request(method, path, body=None, auth=None, trust=context, hostname="127.0.0.1"):
                connection = http.client.HTTPSConnection(hostname, server.server_port, context=trust, timeout=30)
                try:
                    headers = {"Content-Type": "application/octet-stream"}
                    if auth is not None:
                        headers["Authorization"] = auth
                    connection.request(method, path, body=body, headers=headers)
                    response = connection.getresponse()
                    data = response.read(1024 * 1024)
                    return response.status, data
                finally:
                    connection.close()

            auth = credentials(publisher, publisher_password)
            probe = f"/repository/{REPO}/snapshots/auth-check"
            for name, supplied in [("anonymous_upload_refused", None), ("wrong_password_refused", credentials(publisher, "deliberately-wrong"))]:
                status, _ = request("PUT", probe, b"not-authorized", supplied)
                if status not in (401, 403):
                    raise RuntimeError(name + " did not refuse")
                case(name, status=status)
            status, _ = request("PUT", f"/repository/{SCOPE_REPO}/denied", b"not-authorized", auth)
            if status not in (401, 403):
                raise RuntimeError("publisher escaped its repository scope")
            case("publisher_cross_repository_write_refused", status=status)
            for name, trust, hostname in [("untrusted_ca_refused", ssl.create_default_context(), "127.0.0.1"), ("wrong_hostname_refused", context, "localhost")]:
                try:
                    request("HEAD", probe, trust=trust, hostname=hostname)
                except ssl.SSLCertVerificationError:
                    case(name)
                else:
                    raise RuntimeError(name + " did not fail certificate verification")
            result["head_permission_diagnostics"] = {}
            for identity, header in [("administrator", administrator), ("publisher", auth), ("anonymous", None)]:
                status, _ = request("HEAD", f"/repository/{REPO}/snapshots/proof-001/dist/absent.tar.xz", auth=header)
                result["head_permission_diagnostics"][identity] = status
            diagnostic_path = f"/repository/{REPO}/snapshots/proof-001/dist/absent.tar.xz"
            diagnostic = http.client.HTTPConnection("127.0.0.1", nexus_port, timeout=10)
            diagnostic.request("GET", diagnostic_path, headers={"Authorization": administrator})
            direct = diagnostic.getresponse()
            result["direct_raw_diagnostic"] = {"status": direct.status, "body": direct.read(4096).decode("utf-8", errors="replace")}
            diagnostic.close()
            status, body = request("GET", diagnostic_path, auth=administrator)
            result["proxy_raw_diagnostic"] = {"status": status, "body": body.decode("utf-8", errors="replace")[:4096]}
            signature_name = next(n for n, p, sha in files if n.endswith(".crate" if crate_mode else ".asc"))
            signature_bytes = (snapshot / signature_name).read_bytes()
            status, mime_error = request("PUT", f"/repository/{REPO}/snapshots/type-check/" + ("archive.crate" if crate_mode else "signature.asc"), signature_bytes, auth)
            if status == 400:
                case("strict_mime_rejects_original_crate_archive" if crate_mode else "strict_mime_rejects_unchanged_upstream_armored_signature", status=status)
                if not allow_raw_mime_types:
                    result["status"] = "held"
                    result["blocker"] = "Strict MIME validation refuses upstream .asc; no repository policy changed without explicit test opt-in."
                    raise RuntimeError(result["blocker"])
                api("PUT", "repositories/raw/hosted/" + REPO, {"name": REPO, "online": True, "storage": {"blobStoreName": "default", "strictContentTypeValidation": False, "writePolicy": "ALLOW_ONCE"}, "cleanup": {"policyNames": []}, "raw": {"contentDisposition": "ATTACHMENT"}})
                confirmed = api("GET", "repositories/raw/hosted/" + REPO)
                if confirmed["storage"]["strictContentTypeValidation"] is not False or confirmed["storage"]["writePolicy"].lower() != "allow_once":
                    raise RuntimeError("explicit test-repository MIME policy was not retained")
                case("explicit_disposable_raw_mime_policy", strictContentTypeValidation=False, writePolicy=confirmed["storage"]["writePolicy"])
                status, _ = request("PUT", f"/repository/{REPO}/snapshots/type-check/" + ("archive.crate" if crate_mode else "signature.asc"), signature_bytes, auth)
                downloaded_status, downloaded_signature = request("GET", f"/repository/{REPO}/snapshots/type-check/" + ("archive.crate" if crate_mode else "signature.asc"))
                if status not in (200, 201, 204) or downloaded_status != 200 or downloaded_signature != signature_bytes:
                    raise RuntimeError("explicit MIME policy did not preserve the original signature bytes")
                case("explicit_raw_policy_preserves_signature_bytes", status=status)

            elif status in (200, 201, 204):
                case("strict_mime_accepts_original_crate_archive" if crate_mode else "strict_mime_accepts_upstream_signature", status=status)
            else:
                raise RuntimeError(f"unexpected signature MIME probe status: {status}")
            os.environ["NEXUS_USERNAME"] = publisher
            os.environ["NEXUS_PASSWORD"] = publisher_password
            try:
                result["publication"] = publish_snapshot(snapshot, base, ca_file)
                case("authenticated_snapshot_publication_and_get_hashes", files=result["publication"]["files"])
                before = sum(r["method"] == "PUT" for r in requests)
                try:
                    publish_snapshot(snapshot, base, ca_file)
                except ValueError as error:
                    if "destination is not absent" not in str(error):
                        raise
                else:
                    raise RuntimeError("repeat publisher unexpectedly succeeded")
                if sum(r["method"] == "PUT" for r in requests) != before:
                    raise RuntimeError("repeat publisher performed writes")
                case("repeat_publication_refuses_before_put")
            finally:
                os.environ.pop("NEXUS_USERNAME", None)
                os.environ.pop("NEXUS_PASSWORD", None)
            first = next(n for n, p, sha in files if n.endswith(".crate" if crate_mode else ".sha256"))
            original = (snapshot / first).read_bytes()
            status, _ = request("PUT", f"/repository/{REPO}/snapshots/proof-001/{first}", b"replacement", auth)
            if status not in (400, 403, 409, 412):
                raise RuntimeError("Nexus no-redeploy did not refuse overwrite")
            check_status, check_bytes = request("GET", f"/repository/{REPO}/snapshots/proof-001/{first}")
            if check_status != 200 or check_bytes != original:
                raise RuntimeError("overwrite attempt changed the original bytes")
            case("nexus_server_redeploy_refused_and_original_retained", status=status)
            status, _ = request("HEAD", f"/repository/{REPO}/snapshots/proof-001/dist/absent.tar.xz")
            if status != 404:
                raise RuntimeError("missing artifact did not return 404")
            case("missing_artifact_returns_404", status=status)
            status, _ = request("GET", f"/repository/{SCOPE_REPO}/absent")
            if status not in (401, 403):
                raise RuntimeError("anonymous reader escaped repository scope")
            case("anonymous_cross_repository_read_refused", status=status)

            if crate_mode:
                from prove_crates_https import prove_client
                client_evidence = evidence / "cargo-client"
                client_evidence.mkdir()
                result["cargo_client"] = prove_client(client_evidence, snapshot, base, ca_file, cargo, rustc, lambda mode: fault.update(mode=mode))
                case("actual_nexus_cargo_https_fetch_build_locked_and_negative_controls")
                result["status"] = "passed"
                result["tls"] = "Verified temporary CA/IP SAN reverse proxy; actual namespace-local Nexus Raw backend"
                result["limits"] = ["One Linux Cargo itoa client, not every corpus crate compiled", "Only the recorded disposable legacy Nexus image", "Anonymous scoped read, not authenticated Cargo clients", "No production configuration, activation or deletion"]
                return

            # Read-only anonymous client with completely fresh homes and real TLS checks.
            client_request_start = len(requests)
            rustup_version = document["rustup_version"]
            release, host, selector = native_selection(document)
            bootstrap_name = f"rustup/archive/{rustup_version}/{host}/rustup-init"
            bootstrap = evidence / "rustup-init"
            with urlopen(base + "/" + bootstrap_name, context=context, timeout=30) as source, bootstrap.open("xb") as output:
                shutil.copyfileobj(source, output, 1024 * 1024)
            expected = next(sha for n, p, sha in files if n == bootstrap_name)
            if publish_raw.digest(bootstrap) != expected:
                raise RuntimeError("Nexus-served bootstrap identity changed")
            bootstrap.chmod(0o700)
            home = evidence / "client-home"
            home.mkdir()
            env = {"PATH": "/usr/bin:/bin", "HOME": str(home), "CARGO_HOME": str(evidence / "cargo"), "RUSTUP_HOME": str(evidence / "rustup"), "RUSTUP_DIST_SERVER": base, "RUSTUP_UPDATE_ROOT": base + "/rustup", "RUSTUP_INIT_SKIP_PATH_CHECK": "yes", "RUSTUP_INIT_SKIP_SUDO_CHECK": "yes", "RUSTUP_MAX_RETRIES": "0", "RUSTUP_IO_THREADS": "2", "RUSTUP_UNPACK_RAM": "268435456", "CARGO_BUILD_JOBS": "2", "SSL_CERT_FILE": str(ca_file), "CURL_CA_BUNDLE": str(ca_file)}

            def invoke(name, args, cwd=None):
                completed = subprocess.run([str(a) for a in args], env=env, cwd=cwd, capture_output=True, timeout=180)
                (evidence / (name + ".stdout")).write_bytes(completed.stdout)
                (evidence / (name + ".stderr")).write_bytes(completed.stderr)
                result["commands"].append({"case": name, "exit": completed.returncode})
                if completed.returncode:
                    raise RuntimeError("client command failed: " + name)
                return completed

            invoke("bootstrap", [bootstrap, "-y", "--no-modify-path", "--default-host", host, "--default-toolchain", "none"])
            rustup = evidence / "cargo/bin/rustup"
            invoke("disable-self-update", [rustup, "set", "auto-self-update", "disable"])
            invoke("install", [rustup, "toolchain", "install", selector, "--profile", "minimal", "--no-self-update"])
            version = invoke("rustc-version", [rustup, "run", selector, "rustc", "--version"])
            if not version.stdout.startswith(f"rustc {release} ".encode()):
                raise RuntimeError("wrong installed compiler")
            invoke("cargo-version", [rustup, "run", selector, "cargo", "--version"])
            project = evidence / "hello"
            (project / "src").mkdir(parents=True)
            (project / "Cargo.toml").write_text('[package]\nname = "nexus-offline-proof"\nversion = "0.1.0"\nedition = "2021"\n')
            (project / "src/main.rs").write_text('fn main() { println!("nexus offline proof ok"); }\n')
            invoke("cargo-offline-build", [rustup, "run", selector, "cargo", "build", "--offline"], cwd=project)
            program = invoke("run-program", [project / "target/debug/nexus-offline-proof"])
            if program.stdout != b"nexus offline proof ok\n":
                raise RuntimeError("unexpected compiled output")
            if any(r["authenticated"] for r in requests[client_request_start:]):
                raise RuntimeError("client unexpectedly sent credentials")
            case("fresh_anonymous_rustup_bootstrap_and_install_over_trusted_https")
            case("dependency_free_offline_cargo_build_and_run")
            if next_snapshot is not None:
                old_repo, new_repo, active = "rust-mirror-generation-old", "rust-mirror-generation-new", "rust-mirror-active"
                if any(r["name"] in (old_repo, new_repo, active) for r in api("GET", "repositories")):
                    raise RuntimeError("refusing to alter existing generation repositories")
                if not allow_raw_mime_types:
                    raise RuntimeError("new disposable generation repositories require explicit MIME-policy test opt-in")
                for name in (old_repo, new_repo):
                    api("POST", "repositories/raw/hosted", {"name":name,"online":True,"storage":{"blobStoreName":"default","strictContentTypeValidation":False,"writePolicy":"ALLOW_ONCE"},"cleanup":{"policyNames":[]},"raw":{"contentDisposition":"ATTACHMENT"}})
                def group_body(member):
                    return {"name":active,"online":True,"storage":{"blobStoreName":"default","strictContentTypeValidation":False},"group":{"memberNames":[member]}}
                api("POST","repositories/raw/group",group_body(old_repo))
                # These grants affect only the new disposable test repositories.
                for role, actions in [(publisher_role,["browse","read","add","edit"]),(reader_role,["browse","read"])]:
                    current=api("GET","security/roles/"+role)
                    privileges=current["privileges"]+[f"nx-repository-view-raw-{name}-{action}" for name in (old_repo,new_repo) for action in actions]
                    if role==reader_role: privileges += [f"nx-repository-view-raw-{active}-{a}" for a in ("browse","read")]
                    api("PUT","security/roles/"+role,{"id":role,"name":role,"description":"Disposable rust-mirror generation proof","privileges":privileges,"roles":[]})
                server_base=base.split("/repository/")[0]
                os.environ["NEXUS_USERNAME"] = publisher
                os.environ["NEXUS_PASSWORD"] = publisher_password
                try:
                    result["old_generation_publication"]=publish_snapshot(snapshot,server_base+"/repository/"+old_repo,ca_file)
                    result["new_generation_publication"]=publish_snapshot(next_snapshot,server_base+"/repository/"+new_repo,ca_file)
                finally:
                    os.environ.pop("NEXUS_USERNAME",None);os.environ.pop("NEXUS_PASSWORD",None)
                fixed_url=server_base+"/repository/"+active
                def activate():
                    before=api("GET","repositories/raw/group/"+active)
                    if before["group"]["memberNames"] != [old_repo]: raise RuntimeError("unexpected group state before activation")
                    api("PUT","repositories/raw/group/"+active,group_body(new_repo))
                    after=api("GET","repositories/raw/group/"+active)
                    if after["group"]["memberNames"] != [new_repo]: raise RuntimeError("group activation not retained")
                    status,data=request("GET",f"/repository/{active}/snapshot.toml")
                    if status!=200 or hashlib.sha256(data).hexdigest()!=publish_raw.digest(next_snapshot/"snapshot.toml"):
                        raise RuntimeError("group URL did not expose verified new generation")
                    case("fixed_group_url_activation",old_member=old_repo,new_member=new_repo)
                import prove_rustup_lifecycle
                lifecycle=evidence/"lifecycle";lifecycle.mkdir()
                result["lifecycle"]=prove_rustup_lifecycle.exercise(snapshot,next_snapshot,fixed_url,fixed_url,host,lifecycle,ca_file=ca_file,requests=requests,set_fault=lambda mode:fault.update(mode=mode),activate=activate)
                case("actual_nexus_fixed_url_existing_client_update_components_targets_and_self_update")
            result["status"] = "passed"
            result["release"] = release
            result["installed_selector"] = selector
            result["host"] = host
            result["rustup_version"] = rustup_version
            result["tls"] = "Temporary TLS reverse proxy; verified custom CA and IP SAN; HTTP backend on isolated loopback"
            result["limits"] = ["Only recorded Nexus image and Linux/Rustup/Rust combination", "Anonymous read-only Rustup downloads, not authenticated client downloads", "Temporary CA/proxy, not production CA/proxy/SSO configuration", "No production writes or independently verified OpenPGP signatures", ("No automatic client routing, Cargo registry mirroring, GUI or production deployment" if next_snapshot else "No rolling channels, crate/index mirroring, GUI or extra platforms")]
    except Exception:
        if result["status"] != "held":
            result["status"] = "failed"
        raise
    finally:
        os.environ.pop("NEXUS_USERNAME", None)
        os.environ.pop("NEXUS_PASSWORD", None)
        if server is not None:
            server.shutdown()
            server.server_close()
        if worker is not None:
            worker.join(timeout=5)
        (evidence / "requests.json").write_text(json.dumps(requests, indent=2) + "\n")
        (evidence / "summary.json").write_text(json.dumps(result, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--initial-password-file", type=Path, required=True)
    parser.add_argument("--allow-raw-mime-types", action="store_true", help="Opt in to non-strict MIME in this disposable repository only")
    parser.add_argument("--nexus-port", type=int, default=8081)
    parser.add_argument("--next-snapshot", type=Path, help="New immutable generation for full existing-client/self-update proof")
    parser.add_argument("--publisher-cli",type=Path,help="Use the real Rust publish subcommand instead of Python reference publisher")
    parser.add_argument("--cargo", type=Path)
    parser.add_argument("--rustc", type=Path)
    args = parser.parse_args()
    prove(args.snapshot.resolve(), args.evidence.resolve(), args.initial_password_file, args.nexus_port, args.allow_raw_mime_types,
          args.next_snapshot.resolve() if args.next_snapshot else None,args.publisher_cli.resolve() if args.publisher_cli else None, args.cargo.resolve() if args.cargo else None, args.rustc.resolve() if args.rustc else None)


if __name__ == "__main__":
    main()
