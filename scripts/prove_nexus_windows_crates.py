#!/usr/bin/env python3
"""Real crate/client gates for run_nexus_windows.py's owned native instance."""
import contextlib
import hashlib
import http.server
import json
import os
from pathlib import Path
import queue
import secrets
import shutil
import socket
import ssl
import subprocess
import threading
import time
import tomllib
import urllib.request
from prove_crates_https import cargo_paths, command, environment
from prove_lock_import import sources
from prove_nexus import credentials

PROJECT = "abc20a380c8c2d9d76c1976222725d3211cef809"
LOCK_SHA = "3343803df2472c7602d40214bfc42dff094a28ff0a25a7f02cb72ffffea18b9e"


@contextlib.contextmanager
def throttled_connect():
    """Rate-limit encrypted real upstream TLS, never terminate/disable its verification."""
    active = []
    class Proxy(http.server.BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_CONNECT(self):
            if self.path not in ("index.crates.io:443", "static.crates.io:443"):
                self.send_error(403)
                return
            upstream = socket.create_connection((self.path.split(":")[0], 443), timeout=30)
            active.append(upstream)
            stop = threading.Event()
            self.send_response(200)
            self.end_headers()
            def upload():
                try:
                    while not stop.is_set():
                        block = self.connection.recv(65536)
                        if not block:
                            break
                        upstream.sendall(block)
                except OSError:
                    pass
                finally:
                    stop.set()
                    try:
                        upstream.shutdown(socket.SHUT_RDWR)
                    except OSError:
                        pass
            worker = threading.Thread(target=upload, daemon=True)
            worker.start()
            try:
                while not stop.is_set():
                    block = upstream.recv(4096)
                    if not block:
                        break
                    self.connection.sendall(block)
                    stop.wait(0.02)
            except OSError:
                pass  # Deliberate kill closes the client while real TLS is in flight.
            finally:
                stop.set()
                upstream.close()
                self.close_connection = True
                worker.join(timeout=5)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Proxy)
    server.daemon_threads = True
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        yield "http://127.0.0.1:" + str(server.server_port)
    finally:
        server.shutdown()
        server.server_close()
        for connection in active:
            connection.close()
        worker.join(timeout=10)


def invoke(binary, args, evidence, name, env=None, success=True, kill=False):
    argv = [str(binary), "--events", "jsonl", *map(str, args)]
    if not kill:
        p = subprocess.run(argv, env=env, capture_output=True, text=True, timeout=1200)
        events = [json.loads(line) for line in p.stdout.splitlines()]
        assert (p.returncode == 0) == success, (name, p.stderr[-1500:], p.stdout[-1500:])
        assert events[-1]["type"] == "result" and events[-1]["ok"] == success
        assert sum(e["type"] == "result" for e in events) == 1
        stderr = p.stderr
    else:
        process = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
        lines = queue.Queue()
        def consume():
            for line in process.stdout:
                lines.put(line)
            lines.put(None)
        reader = threading.Thread(target=consume, daemon=True)
        reader.start()
        events = []
        stopped = False
        deadline = time.monotonic() + 180
        try:
            while time.monotonic() < deadline:
                line = lines.get(timeout=max(0.1, deadline-time.monotonic()))
                if line is None:
                    break
                event = json.loads(line)
                events.append(event)
                if not stopped and event["type"] == "bytes" and event["path"].endswith(".crate") and event["count"] >= 65536:
                    process.kill()  # TerminateProcess on Windows, no graceful-cancel substitute.
                    stopped = True
            assert stopped and process.wait(timeout=15) != 0, "hard kill not exercised"
            stderr = process.stderr.read()
            assert not any(e["type"] == "result" and e["ok"] for e in events)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=15)
            reader.join(timeout=5)
            process.stdout.close()
            process.stderr.close()
    assert events[0]["type"] == "hello" and events[0]["protocol"] == 1
    assert [e["seq"] for e in events] == list(range(len(events)))
    (evidence / (name + ".jsonl")).write_text("".join(json.dumps(e) + "\n" for e in events))
    (evidence / (name + ".stderr")).write_text(stderr)
    return events


def prove(root, evidence, binary, request, repo, port, ca, requests, result, with_toolchain=False):
    def case(name, **values):
        result["cases"].append({"case": name, "passed": True, **values})
        print("passed:", name, flush=True)
    def api(method, path, data=None, text=False):
        status, body = request(method, "/service/rest/v1/" + path, data, text=text)
        assert 200 <= status < 300, (method, path, status)
        return json.loads(body) if body.strip() else None
    suffix = repo.removeprefix("rust-mirror-win-")
    repositories = [repo]
    if with_toolchain:
        tool_repo = repo + "-toolchains"
        api("POST", "repositories/raw/hosted", {"name": tool_repo, "online": True, "storage": {"blobStoreName": "default", "strictContentTypeValidation": False, "writePolicy": "ALLOW_ONCE"}, "cleanup": {"policyNames": []}, "raw": {"contentDisposition": "ATTACHMENT"}})
        repositories.append(tool_repo)
    for role, actions in [("publish-"+suffix, ["browse", "read", "add", "edit"]), ("read-"+suffix, ["browse", "read"])]:
        api("POST", "security/roles", {"id": role, "name": role, "description": "Owned native Windows proof", "privileges": [f"nx-repository-view-raw-{r}-{a}" for r in repositories for a in actions], "roles": []})
    user = "publisher-" + suffix
    password = secrets.token_urlsafe(32)
    reader_password = secrets.token_urlsafe(32)
    for name, role, pw in [(user, "publish-"+suffix, password), ("reader-"+suffix, "read-"+suffix, reader_password)]:
        api("POST", "security/users", {"userId": name, "firstName": "Windows", "lastName": "Proof", "emailAddress": "mirror@example.invalid", "password": pw, "status": "active", "roles": [role]})
    api("PUT", "security/anonymous", {"enabled": True, "userId": "reader-"+suffix, "realmName": "NexusAuthorizingRealm"})
    replacement = secrets.token_urlsafe(32)
    api("PUT", "security/users/admin/change-password", replacement, text=True)
    case("random_admin_rotation_scoped_publisher_and_anonymous_read_only")
    auth = credentials(user, password)
    origin = f"https://127.0.0.1:{port}"
    base = origin + "/repository/" + repo + "/crate-generation"
    ca_file = root / "public-ca.pem"
    shutil.copyfile(ca, ca_file)
    publish_env = {**os.environ, "NEXUS_USERNAME": user, "NEXUS_PASSWORD": password}
    cargo, rustc = cargo_paths()
    corpus = [{"repo": "sharkdp/hexyl", "commit": PROJECT, "name": "hexyl", "lock_sha256": LOCK_SHA, "unextracted_links": []}]
    sources(root, corpus)
    project = root / "projects/hexyl"
    manifest = tomllib.loads((project / "Cargo.toml").read_text())
    assert {"assert_cmd", "predicates", "pretty_assertions"} <= set(manifest["dev-dependencies"])
    lock = (project / "Cargo.lock").read_bytes()
    catalog = root / "crates.toml"
    catalog.write_text('[crates]\nserde={version="1",features=["derive"]}\nserde_json="1"\nanyhow="1"\n[crate-policy]\nhosts=["x86_64-pc-windows-msvc"]\ntargets=["x86_64-pc-windows-msvc"]\n[crate-output]\nbase-url='+json.dumps(base)+'\n[crate-network]\ncontact="https://github.com/rustcommons/rust-mirror/issues"\n')
    imported = invoke(binary, ["crates", "import-lock", "--config", catalog, "--lock", "hexyl="+str(project/"Cargo.lock"), "--write"], evidence, "import-lock")[-1]["report"]
    assert imported["accepted"] and imported["written"] and imported["external_sources_required"]
    assert imported["sources"][0]["skipped"] == [{"column":1,"kind":"local","line":218,"name":"hexyl","version":"0.16.0"}]
    run = root / "crate-run"
    preview_events = invoke(binary, ["crates", "sync", "--config", catalog, "--output", run], evidence, "sync-preview")
    preview = preview_events[-1]["report"]
    assert preview["status"] == "needs_approval" and not (run / "mirror").exists()
    assert not any(e["type"] == "artifact" and e.get("path", "").endswith(".crate") for e in preview_events)
    plan_id = preview["plan_id"]
    shown = invoke(binary, ["crates", "show", "--plan", run/"plan.json"], evidence, "show-plan")[-1]["report"]
    assert shown == preview["preview"]
    assert {"assert_cmd", "predicates", "pretty_assertions"} <= {v["name"] for v in shown["versions"]}
    case("real_catalog_and_dev_dependency_lock_import_archive_free_preview", plan_id=plan_id, versions=len(shown["versions"]))
    with throttled_connect() as proxy:
        args = ["crates", "sync", "--config", catalog, "--output", run, "--approve-plan", plan_id, "--proxy", proxy]
        invoke(binary, args, evidence, "sync-hard-kill", kill=True)
        partials = [p for p in (run/".download/files").glob("*.part") if p.stat().st_size > 0]
        assert partials and not (run/"mirror").exists()
        partial_bytes = [p.stat().st_size for p in partials]
        resumed = invoke(binary, [*args, "--resume"], evidence, "sync-resumed")
    assert resumed[-1]["report"]["status"] == "completed" and resumed[-1]["report"]["plan_id"] == plan_id
    assert any(e["type"] == "artifact" and e.get("action") == "resume" and e.get("offset", 0) > 0 for e in resumed)
    mirror = run / "mirror"
    inventory = tomllib.loads((mirror/"snapshot.toml").read_text())
    assert inventory["schema"] == 5
    for file in inventory["files"]:
        b = (mirror/file["path"]).read_bytes()
        assert len(b)==file["bytes"] and hashlib.sha256(b).hexdigest()==file["sha256"]
    case("actual_windows_hard_kill_mid_archive_and_frozen_resume", partial_bytes=partial_bytes, files=len(inventory["files"]))
    publish = ["publish", "--source", mirror, "--base-url", base, "--ca-file", ca_file]
    puts = lambda: sum(r["method"] in ("PUT","POST","DELETE","PATCH") and r["path"].startswith('/repository/') for r in requests)
    before = puts()
    invoke(binary, [*publish, "--preflight"], evidence, "publish-preflight", env=publish_env)
    assert puts()==before
    invoke(binary, publish, evidence, "publish", env=publish_env)
    assert puts()-before == len(inventory["files"])+1
    before = puts()
    repeat = invoke(binary, publish, evidence, "publish-repeat", env=publish_env, success=False)
    assert repeat[-1]["error"]["kind"]=="remote_exists" and puts()==before
    case("native_publish_preflight_no_put_publish_and_repeat_no_put")
    artifact = next(f for f in inventory["files"] if f["path"].endswith('.crate'))
    path = "/repository/"+repo+"/crate-generation/"+artifact["path"]
    status, _ = request("PUT", path, "refused overwrite", auth=auth, text=True)
    assert status==400, status
    status, b = request("GET", path, auth=None)
    assert status==200 and hashlib.sha256(b).hexdigest()==artifact["sha256"]
    status, _ = request("PUT", "/repository/"+repo+"/anonymous-write", "x", auth=None, text=True)
    assert status in (401,403)
    case("nexus_server_overwrite_and_anonymous_write_refused_original_retained")
    client_home = root / "client-cargo-home"
    client_home.mkdir()
    (client_home/"config.toml").write_text('[source.crates-io]\nreplace-with="owned-nexus"\n[source.owned-nexus]\nregistry='+json.dumps('sparse+'+base+'/cargo/index/')+'\n')
    client_env = environment(client_home, root/"client-target", rustc)
    client_env.update(CARGO_HTTP_CAINFO=str(ca_file), CARGO_HTTP_SSL_VERIFY="true")
    for name, options in [("cargo-fetch-locked", ["fetch", "--locked"]), ("cargo-build-locked", ["build", "--locked"]), ("cargo-build-locked-offline", ["build", "--locked", "--offline"])]:
        if "offline" in name:
            client_env["CARGO_TARGET_DIR"] = str(root/"client-offline-target")
        p = command([cargo, *options], cwd=project, env=client_env, timeout=600)
        (evidence/(name+'.stdout')).write_text(p.stdout)
        (evidence/(name+'.stderr')).write_text(p.stderr)
        assert (project/"Cargo.lock").read_bytes()==lock
    served_crates = [r["path"] for r in requests if r["method"]=="GET" and r["status"]==200 and r["path"].endswith('.crate')]
    assert any('/assert_cmd/' in path for path in served_crates), "dev package not fetched from actual Nexus"
    case("real_windows_cargo_https_fetch_build_and_fresh_target_offline_build", project="sharkdp/hexyl", commit=PROJECT, lock_sha256=LOCK_SHA, dev_dependencies=sorted(manifest['dev-dependencies']), archive_gets=len(served_crates), unchanged_lock=True)
    result["crate_inventory"] = {"schema":5,"sha256":hashlib.sha256((mirror/'snapshot.toml').read_bytes()).hexdigest(),"versions":len(shown['versions']),"archive_bytes":sum(f['bytes'] for f in inventory['files'] if f['path'].endswith('.crate'))}
    result["client"] = {"cargo":command([cargo,'--version']).stdout.strip(),"rustc":command([rustc,'-vV']).stdout,"source_replacement":True,"ca_only":True,"tls_verification":True,"air_gap_simulation":False}
    result["toolchain"] = {"covered":False,"reason":"Separate optional proof not selected"}
    if with_toolchain:
        toolchain(root,evidence,binary,origin+'/repository/'+tool_repo+'/toolchain-generation',ca_file,publish_env,result)
    # Evidence contains only public URLs and sanitized protocol/command output.
    for p in evidence.iterdir():
        if p.is_file():
            b=p.read_bytes()
            assert all(secret.encode() not in b for secret in [password,reader_password,replacement]), "secret in retained evidence"


def toolchain(root,evidence,binary,base,ca,env,result):
    cfg=root/'toolchains.toml'
    cfg.write_text('schema=1\nrustup_version="1.28.2"\n[[selections]]\nname="windows"\ntoolchain="1.90.0"\nhosts=["x86_64-pc-windows-msvc"]\nprofile="minimal"\n')
    plan=root/'toolchain-plan';fetched=root/'toolchain-fetched';finished=root/'toolchain-finished'
    invoke(binary,['plan','--config',cfg,'--selection','windows','--output',plan],evidence,'toolchain-plan')
    invoke(binary,['fetch','--plan',plan,'--output',fetched],evidence,'toolchain-fetch')
    invoke(binary,['rustup','--snapshot',fetched/'mirror','--version','1.28.2','--output',finished],evidence,'toolchain-rustup')
    invoke(binary,['publish','--source',finished/'mirror','--base-url',base,'--ca-file',ca,'--preflight'],evidence,'toolchain-preflight',env=env)
    invoke(binary,['publish','--source',finished/'mirror','--base-url',base,'--ca-file',ca],evidence,'toolchain-publish',env=env)
    trust=ssl.create_default_context(cafile=str(ca));bootstrap=root/'rustup-init.exe'
    with urllib.request.urlopen(base+'/rustup/archive/1.28.2/x86_64-pc-windows-msvc/rustup-init.exe',context=trust,timeout=60) as response:
        bootstrap.write_bytes(response.read(64*1024*1024+1))
    assert hashlib.sha256(bootstrap.read_bytes()).digest()==hashlib.sha256((finished/'mirror/rustup/archive/1.28.2/x86_64-pc-windows-msvc/rustup-init.exe').read_bytes()).digest()
    client={k:v for k,v in os.environ.items() if not k.startswith(('CARGO_','RUSTUP_')) and k not in ('RUSTFLAGS','RUSTDOCFLAGS')}
    client.update(HOME=str(root/'rustup-user'),USERPROFILE=str(root/'rustup-user'),CARGO_HOME=str(root/'rustup-cargo'),RUSTUP_HOME=str(root/'rustup-home'),RUSTUP_DIST_SERVER=base,RUSTUP_UPDATE_ROOT=base+'/rustup',RUSTUP_INIT_SKIP_PATH_CHECK='yes',RUSTUP_INIT_SKIP_SUDO_CHECK='yes',RUSTUP_NO_WINDOWS_PATH_WARNING='1',RUSTUP_MAX_RETRIES='0',RUSTUP_IO_THREADS='2',RUSTUP_UNPACK_RAM='268435456',SSL_CERT_FILE=str(ca),CURL_CA_BUNDLE=str(ca))
    Path(client['HOME']).mkdir()
    for name,argv in [('rustup-bootstrap',[bootstrap,'-y','--no-modify-path','--default-host','x86_64-pc-windows-msvc','--default-toolchain','none']),('rustup-install',[root/'rustup-cargo/bin/rustup.exe','toolchain','install','1.90.0','--profile','minimal','--no-self-update']),('rustup-rustc',[root/'rustup-cargo/bin/rustup.exe','run','1.90.0','rustc','--version'])]:
        p=command(argv,env=client,timeout=600)
        (evidence/(name+'.stdout')).write_text(p.stdout);(evidence/(name+'.stderr')).write_text(p.stderr)
        if name=='rustup-rustc':assert 'rustc 1.90.0' in p.stdout
    result['toolchain']={'covered':True,'rust':'1.90.0','rustup':'1.28.2','profile':'minimal','host':'x86_64-pc-windows-msvc','same_native_nexus':True,'separate_owned_raw_repository':True,'mime_opt_in':True,'scope':'One fresh minimal Rustup install and rustc version, not the full lifecycle matrix'}
