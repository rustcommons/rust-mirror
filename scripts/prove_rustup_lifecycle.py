#!/usr/bin/env python3
"""Exercise real Rustup clients against old/new verified generations in private homes.

Linux runs require a loopback-only network namespace. macOS requires sandbox-exec.
Windows requires --windows-firewall-proof (disposable CI runner, configured by workflow).
No user Rustup/Cargo homes, shell profiles, production repository writes or EULAs.
"""
import argparse
import http.server
import json
import os
from pathlib import Path
import platform
import shutil
import socket
import ssl
import hashlib
import subprocess
import threading
import time
import tomllib
from urllib.parse import urlsplit, unquote
import publish_raw


def isolation(evidence, windows_firewall_proof=None):
    if platform.system() == "Linux":
        routes = json.loads(subprocess.check_output(["ip", "-json", "route"]))
        interfaces = json.loads(subprocess.check_output(["ip", "-json", "address"]))
        if routes or {i["ifname"] for i in interfaces} != {"lo"}:
            raise RuntimeError("Linux lifecycle proof requires a loopback-only network namespace")
        return {"kind": "network-namespace", "routes": routes}, []
    if platform.system() == "Darwin":
        sandbox = Path("/usr/bin/sandbox-exec")
        if not sandbox.is_file():
            raise RuntimeError("native macOS proof requires sandbox-exec network isolation")
        policy = '(version 1)(allow default)(deny network-outbound)(allow network-outbound (remote ip "localhost:*"))'
        return {"kind": "sandbox-exec", "policy": policy}, [str(sandbox), "-p", policy]
    if platform.system() == "Windows":
        if windows_firewall_proof is None:
            raise RuntimeError("Windows native proof requires disposable-runner firewall evidence")
        doc = json.loads(windows_firewall_proof.read_text(encoding="utf-8-sig"))
        if doc.get("disposable_runner") is not True or not doc.get("rules"):
            raise RuntimeError("invalid Windows firewall evidence")
        return {"kind": "program-scoped-firewall", "evidence": doc}, []
    raise RuntimeError("no native isolation contract for this platform")


def exercise(old, new, old_url, new_url, host, evidence, ca_file=None, prefix=(), requests=None, set_fault=None):
    old_doc, old_files = publish_raw.local_files(old)
    new_doc, new_files = publish_raw.local_files(new)
    old_manifest = tomllib.loads((old / "dist/channel-rust-stable.toml").read_text(encoding="utf-8"))
    new_manifest = tomllib.loads((new / "dist/channel-rust-stable.toml").read_text(encoding="utf-8"))
    old_version = old_manifest["pkg"]["rust"]["version"].split()[0]
    new_version = new_manifest["pkg"]["rust"]["version"].split()[0]
    if old_version == new_version:
        raise ValueError("lifecycle proof requires two different stable releases")
    suffix = ".exe" if os.name == "nt" else ""
    bootstrap_rel = f"rustup/archive/{old_doc['rustup_version']}/{host}/rustup-init{suffix}"
    bootstrap = evidence / ("rustup-init" + suffix)
    shutil.copyfile(old / bootstrap_rel, bootstrap)
    bootstrap.chmod(0o700)
    home = evidence / "home"
    home.mkdir()
    cargo = evidence / "cargo"
    rustup_home = evidence / "rustup"
    env = {k:v for k,v in os.environ.items() if not k.startswith(("RUSTUP_", "CARGO_", "RUSTC_")) and k not in ("RUSTFLAGS", "RUSTDOCFLAGS")}
    for key in list(env):
        if key.lower() in ("http_proxy","https_proxy","all_proxy","no_proxy"): env.pop(key)
    env.update(NO_PROXY="127.0.0.1,localhost", no_proxy="127.0.0.1,localhost")
    env.update(HOME=str(home), USERPROFILE=str(home), CARGO_HOME=str(cargo), RUSTUP_HOME=str(rustup_home), RUSTUP_DIST_SERVER=old_url,
               RUSTUP_UPDATE_ROOT=old_url+"/rustup", RUSTUP_INIT_SKIP_PATH_CHECK="yes", RUSTUP_INIT_SKIP_SUDO_CHECK="yes", RUSTUP_MAX_RETRIES="0",
               RUSTUP_IO_THREADS="2", RUSTUP_UNPACK_RAM="268435456", CARGO_BUILD_JOBS="2", RUSTUP_NO_WINDOWS_PATH_WARNING="1")
    if ca_file:
        env.update(SSL_CERT_FILE=str(ca_file), CURL_CA_BUNDLE=str(ca_file))
    result = {"status":"running", "host":host, "old_rust":old_version, "new_rust":new_version, "old_rustup":old_doc["rustup_version"], "new_rustup":new_doc["rustup_version"], "commands":[]}
    rustup = cargo / "bin" / ("rustup"+suffix)
    def invoke(name, args, expect=0, cwd=None):
        p = subprocess.run([*prefix,*map(str,args)],env=env,cwd=cwd,capture_output=True,timeout=300)
        (evidence/(name+".stdout")).write_bytes(p.stdout)
        (evidence/(name+".stderr")).write_bytes(p.stderr)
        result["commands"].append({"case":name,"exit":p.returncode,"expected_success":expect==0})
        if (p.returncode==0) != (expect==0):
            raise RuntimeError(f"lifecycle case failed: {name}; see retained stderr")
        return p.stdout
    def payload_count():
        return sum(r["path"].split("?")[0].endswith((".tar.gz",".tar.xz",".tar.zst")) for r in requests or [])
    try:
        if os.name != "nt":
            shell=old/"rustup/rustup-init.sh"
            invoke("shell-bootstrap",["sh",shell,"-y","--no-modify-path","--default-host",host,"--default-toolchain","none"])
        else:
            invoke("bootstrap",[bootstrap,"-y","--no-modify-path","--default-host",host,"--default-toolchain","none"])
        initial=invoke("old-rustup-version",[rustup,"--version"])
        if old_doc["rustup_version"].encode() not in initial: raise RuntimeError("wrong bootstrap version")
        invoke("disable-automatic-self-update",[rustup,"set","auto-self-update","disable"])
        invoke("install-old-stable",[rustup,"toolchain","install","stable","--profile","minimal","--no-self-update"])
        invoke("default-stable",[rustup,"default","stable"])
        compiler=invoke("old-compiler",[rustup,"run","stable","rustc","--version"])
        if not compiler.startswith(("rustc "+old_version+" ").encode()): raise RuntimeError("wrong old compiler")
        dated="stable-"+old_manifest["date"]
        invoke("install-old-dated",[rustup,"toolchain","install",dated,"--profile","minimal","--no-self-update"])
        # Existing CARGO_HOME/RUSTUP_HOME survive promotion to the next immutable generation.
        env.update(RUSTUP_DIST_SERVER=new_url,RUSTUP_UPDATE_ROOT=new_url+"/rustup")
        invoke("update-existing-stable",[rustup,"update","stable","--no-self-update"])
        compiler=invoke("new-compiler",[rustup,"run","stable","rustc","--version"])
        if not compiler.startswith(("rustc "+new_version+" ").encode()): raise RuntimeError("existing stable did not update")
        before=payload_count()
        invoke("repeat-stable-update",[rustup,"update","stable","--no-self-update"])
        if requests is not None and payload_count()!=before: raise RuntimeError("unchanged update fetched toolchain archives")
        invoke("component-add",[rustup,"component","add","clippy","rustfmt","rust-src","--toolchain","stable"])
        invoke("target-add",[rustup,"target","add","wasm32-unknown-unknown","--toolchain","stable"])
        invoke("clippy-version",[rustup,"run","stable","cargo-clippy","--version"])
        invoke("rustfmt-version",[rustup,"run","stable","rustfmt","--version"])
        installed=invoke("target-list",[rustup,"target","list","--installed","--toolchain","stable"])
        if b"wasm32-unknown-unknown" not in installed: raise RuntimeError("missing selected target")
        invoke("component-remove",[rustup,"component","remove","clippy","--toolchain","stable"])
        invoke("component-readd",[rustup,"component","add","clippy","--toolchain","stable"])
        invoke("target-remove",[rustup,"target","remove","wasm32-unknown-unknown","--toolchain","stable"])
        invoke("target-readd",[rustup,"target","add","wasm32-unknown-unknown","--toolchain","stable"])
        invoke("unavailable-component-refuses",[rustup,"component","add","miri","--toolchain","stable"],expect=1)
        project=evidence/"hello"
        (project/"src").mkdir(parents=True)
        (project/"Cargo.toml").write_text('[package]\nname="rustup-lifecycle-proof"\nversion="0.1.0"\nedition="2021"\n',encoding="utf-8")
        (project/"src/main.rs").write_text('fn main() { println!("rustup lifecycle ok"); }\n',encoding="utf-8")
        invoke("cargo-offline-build",[rustup,"run","stable","cargo","build","--offline"],cwd=project)
        output=invoke("run-built-program",[project/"target/debug"/("rustup-lifecycle-proof"+suffix)])
        if output.strip()!=b"rustup lifecycle ok": raise RuntimeError("wrong offline program output")
        invoke("wasm-offline-check",[rustup,"run","stable","cargo","check","--offline","--target","wasm32-unknown-unknown"],cwd=project)
        invoke("dated-remove",[rustup,"toolchain","uninstall",dated])
        invoke("dated-reinstall",[rustup,"toolchain","install",dated,"--profile","minimal","--no-self-update"])
        invoke("self-update",[rustup,"self","update"])
        # Windows replacement finishes in a child; only private proof executables are retried.
        deadline=time.monotonic()+30
        while True:
            try:
                output=invoke("new-rustup-version",[rustup,"--version"])
                if new_doc["rustup_version"].encode() in output: break
            except (OSError,RuntimeError):
                if os.name != "nt": raise
            if time.monotonic()>deadline: raise RuntimeError("self-update did not install selected version")
            time.sleep(0.2)
        invoke("repeat-self-update",[rustup,"self","update"])
        compiler=invoke("post-self-update-compiler",[rustup,"run","stable","rustc","--version"])
        if not compiler.startswith(("rustc "+new_version+" ").encode()): raise RuntimeError("self-update changed toolchain selection")
        if set_fault:
            for mode, expected in [("missing", b"404"),("corrupt", b"checksum")]:
                env["RUSTUP_HOME"]=str(evidence/("negative-"+mode))
                set_fault(mode)
                try:
                    invoke("negative-"+mode,[rustup,"toolchain","install","stable","--profile","minimal","--no-self-update"],expect=1)
                    message=(evidence/("negative-"+mode+".stderr")).read_bytes().lower()
                    if expected not in message: raise RuntimeError("negative control failed for unrelated reason: "+mode)
                finally:
                    set_fault(None)
            env["RUSTUP_HOME"]=str(rustup_home)
        result["extra_channels"]=[]
        for channel in ["beta", "nightly"]:
            manifest=new/f"dist/channel-rust-{channel}.toml"
            if manifest.exists():
                version=tomllib.loads(manifest.read_text(encoding="utf-8"))["pkg"]["rust"]["version"].split()[0]
                invoke(channel+"-install",[rustup,"toolchain","install",channel,"--profile","minimal","--no-self-update"])
                compiler=invoke(channel+"-compiler",[rustup,"run",channel,"rustc","--version"])
                if not compiler.startswith(("rustc "+version+" ").encode()): raise RuntimeError("wrong extra channel compiler")
                before=payload_count()
                invoke(channel+"-repeat",[rustup,"update",channel,"--no-self-update"])
                if requests is not None and payload_count()!=before: raise RuntimeError("unchanged extra channel fetched archives")
                result["extra_channels"].append({"channel":channel,"version":version})
        invoke("check-current-channels",[rustup,"check"])
        result["status"]="passed"
        result["limits"]=["Selected native host and exact versions only", "Original HTTPS/checksum trust; no independent signature verification", "Cargo dependencies/SDKs are not mirrored by this proof"]
    finally:
        (evidence/"lifecycle.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
    return result


def fixture_tls(evidence):
    openssl=shutil.which("openssl")
    if openssl is None and os.name=="nt":
        candidate=Path(os.environ.get("ProgramFiles","C:/Program Files"))/"Git/usr/bin/openssl.exe"
        if candidate.is_file(): openssl=str(candidate)
    if openssl is None: raise RuntimeError("TLS fixture requires preinstalled openssl")
    cert=evidence/"test-ca.pem"; key=evidence/"server.key"
    config=evidence/"ca.cnf"
    config.write_text("[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ca\n[dn]\nCN=rust-mirror-lifecycle-ca\n[ca]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid:always\n",encoding="utf-8")
    commands=[
        [openssl,"req","-x509","-newkey","rsa:2048","-nodes","-days","1","-config",str(config),"-keyout",str(evidence/"ca.key"),"-out",str(cert)],
        [openssl,"req","-new","-newkey","rsa:2048","-nodes","-subj","/CN=127.0.0.1","-keyout",str(key),"-out",str(evidence/"server.csr")],
    ]
    extensions=evidence/"server.ext"
    extensions.write_text("subjectAltName=IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\n",encoding="utf-8")
    commands.append([openssl,"x509","-req","-in",str(evidence/"server.csr"),"-CA",str(cert),"-CAkey",str(evidence/"ca.key"),"-CAcreateserial","-out",str(evidence/"server.pem"),"-days","1","-extfile",str(extensions)])
    for command in commands:
        p=subprocess.run(command,capture_output=True)
        if p.returncode: raise RuntimeError("temporary TLS certificate generation failed")
    context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);context.load_cert_chain(evidence/"server.pem",key)
    cleanup=lambda: None
    if platform.system() in ("Windows","Darwin"):
        if os.environ.get("GITHUB_ACTIONS")!="true": raise RuntimeError("native trust-store tests are restricted to disposable CI runners")
        if os.name=="nt":
            script=evidence/"import-test-ca.ps1"
            script.write_text('param([string]$CertPath)\n$ErrorActionPreference="Stop"\n$c=Import-Certificate -FilePath $CertPath -CertStoreLocation Cert:/CurrentUser/Root\n$c.Thumbprint\n',encoding="utf-8")
            imported=subprocess.run(["powershell.exe","-NoProfile","-File",str(script),str(cert)],capture_output=True,text=True,check=True)
            thumb=imported.stdout.strip().splitlines()[-1]
            if not thumb or any(c not in "0123456789ABCDEFabcdef" for c in thumb): raise RuntimeError("unexpected certificate identity")
            cleanup=lambda: subprocess.run(["powershell.exe","-NoProfile","-Command",f"Remove-Item -LiteralPath Cert:/CurrentUser/Root/{thumb}"],check=True,capture_output=True)
        else:
            # CI's login keychain only; delete this exact certificate in finally.
            keychain=str(Path.home()/"Library/Keychains/login.keychain-db")
            subprocess.run(["security","add-trusted-cert","-r","trustRoot","-k",keychain,str(cert)],capture_output=True,check=True)
            der=ssl.PEM_cert_to_DER_cert(cert.read_text())
            fingerprint=hashlib.sha1(der).hexdigest().upper()
            cleanup=lambda: subprocess.run(["security","delete-certificate","-Z",fingerprint,"-t",keychain],capture_output=True,check=True)
    return context,cert,cleanup

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--old",type=Path,required=True)
    p.add_argument("--new",type=Path,required=True)
    p.add_argument("--host",required=True)
    p.add_argument("--evidence",type=Path,required=True)
    p.add_argument("--windows-firewall-proof",type=Path)
    args=p.parse_args()
    args.evidence=args.evidence.resolve();args.evidence.mkdir(parents=True,exist_ok=False)
    detail,prefix=isolation(args.evidence,args.windows_firewall_proof)
    (args.evidence/"isolation.json").write_text(json.dumps(detail,indent=2)+"\n",encoding="utf-8")
    roots={"old":args.old.resolve(),"new":args.new.resolve()}; requests=[]; fault={"mode":None}
    for root in roots.values(): publish_raw.local_files(root)
    class Handler(http.server.SimpleHTTPRequestHandler):
        def do_GET(self):
            if fault["mode"] and urlsplit(self.path).path.endswith((".tar.gz",".tar.xz",".tar.zst")):
                if fault["mode"]=="missing": self.send_error(404); return
                data=b"deliberately corrupt archive"
                self.send_response(200);self.send_header("Content-Length",str(len(data)));self.end_headers();self.wfile.write(data);return
            super().do_GET()
        def log_message(self,*a): pass
        def translate_path(self,path):
            parts=unquote(urlsplit(path).path).split("/")
            if len(parts)<3 or parts[1] not in roots or any(v in (".","..") for v in parts): return str(args.evidence/"absent")
            return str(roots[parts[1]].joinpath(*parts[2:]))
        def log_request(self,code="-",size="-"): requests.append({"method":self.command,"path":self.path,"status":str(code)})
    context,ca_file,cleanup=fixture_tls(args.evidence)
    server=None;thread=None
    try:
        server=http.server.ThreadingHTTPServer(("127.0.0.1",0),Handler)
        server.socket=context.wrap_socket(server.socket,server_side=True)
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        base=f"https://127.0.0.1:{server.server_port}"
        exercise(args.old,args.new,base+"/old",base+"/new",args.host,args.evidence,ca_file=ca_file,prefix=prefix,requests=requests,set_fault=lambda mode:fault.update(mode=mode))
    finally:
        if server:
            if thread and thread.is_alive(): server.shutdown()
            server.server_close()
        if thread: thread.join(timeout=5)
        try: cleanup()
        finally: (args.evidence/"requests.json").write_text(json.dumps(requests,indent=2)+"\n",encoding="utf-8")

if __name__=="__main__": main()
