#!/usr/bin/env python3
"""Launch an owned, network-disabled Nexus container and run the integration proof.

Linux root only. Docker or rootful Podman; image must already be pulled. Does not
connect to a production Nexus server. Removes only its own container and volume.
"""
import argparse
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import time

LABEL = "org.rustcommons.rust-mirror-proof"


def run(runtime, args, **kwargs):
    return subprocess.run([runtime, *args], capture_output=True, check=True, timeout=180, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", choices=("podman", "docker"), default="podman")
    parser.add_argument("--image", required=True, help="Explicit already-pulled test image; no implicit server version selection")
    parser.add_argument("--allow-raw-mime-types", action="store_true", help="Opt in to arbitrary MIME types in the new disposable Raw repository only")
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--next-snapshot", type=Path, help="New generation for native client update/self-update qualification")
    parser.add_argument("--publisher-cli",type=Path,help="Real Rust publish CLI for the actual Nexus proof")
    args = parser.parse_args()
    if os.geteuid() != 0:
        parser.error("run as root for rootful runtime and namespace entry")
    runtime = shutil.which(args.runtime)
    if runtime is None or shutil.which("nsenter") is None:
        parser.error("selected runtime and nsenter must already be installed")
    args.evidence = args.evidence.resolve()
    args.evidence.mkdir(parents=True, exist_ok=False, mode=0o700)
    initial = args.evidence / "initial-password.secret"
    container_id = None
    volume = None
    suffix = secrets.token_hex(5)
    name = "rust-mirror-nexus-" + suffix
    state = {"status": "starting", "image": args.image, "runtime": args.runtime}
    try:
        image = json.loads(run(runtime, ["image", "inspect", args.image]).stdout)[0]
        state["image_id"] = image.get("Id", image.get("ID"))
        state["image_digests"] = image.get("RepoDigests", [])
        # Pulling is separate and visible; never silently replace a pinned image here.
        volume = name + "-data"
        run(runtime, ["volume", "create", "--label", LABEL + "=" + suffix, volume])
        container_id = run(runtime, ["run", "--detach", "--pull=never", "--name", name, "--label", LABEL + "=" + suffix, "--network", "none", "--memory", "2g", "--cpus", "2", "--volume", volume + ":/nexus-data", "--env", "INSTALL4J_ADD_VM_PARAMS=-Xms256m -Xmx768m -XX:MaxDirectMemorySize=256m -Djava.util.prefs.userRoot=/nexus-data/javaprefs", args.image]).stdout.decode().strip()
        if not container_id or any(c not in "0123456789abcdef" for c in container_id):
            raise RuntimeError("unexpected container identity")
        inspect = json.loads(run(runtime, ["inspect", container_id]).stdout)[0]
        state["container_id"] = container_id
        if inspect["Config"]["Labels"].get(LABEL) != suffix or inspect["HostConfig"]["NetworkMode"] != "none":
            raise RuntimeError("owned container/network isolation check failed")
        pid = inspect["State"]["Pid"]
        print("Nexus starting in its isolated container; waiting for initial password.", flush=True)
        deadline = time.monotonic() + 420
        while time.monotonic() < deadline:
            check = subprocess.run([runtime, "exec", container_id, "cat", "/nexus-data/admin.password"], capture_output=True, timeout=15)
            if check.returncode == 0 and check.stdout.strip():
                descriptor = os.open(initial, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                with os.fdopen(descriptor, "wb") as f:
                    f.write(check.stdout.strip())
                break
            current = json.loads(run(runtime, ["inspect", container_id]).stdout)[0]
            if not current["State"]["Running"]:
                raise RuntimeError("Nexus stopped before initialization")
            time.sleep(2)
        else:
            raise TimeoutError("Nexus initialization timed out")
        command = ["nsenter", "--target", str(pid), "--net", "python3", str(Path(__file__).with_name("prove_nexus.py").resolve()), "--snapshot", str(args.snapshot.resolve()), "--evidence", str(args.evidence / "proof"), "--initial-password-file", str(initial)]
        if args.publisher_cli:
            command.extend(["--publisher-cli",str(args.publisher_cli.resolve())])
        if args.next_snapshot:
            command.extend(["--next-snapshot", str(args.next_snapshot.resolve())])
        if args.allow_raw_mime_types:
            command.append("--allow-raw-mime-types")
        proof = subprocess.run(command, capture_output=True, timeout=900)
        (args.evidence / "driver.stdout").write_bytes(proof.stdout)
        (args.evidence / "driver.stderr").write_bytes(proof.stderr)
        state["proof_exit"] = proof.returncode
        if proof.returncode:
            raise RuntimeError("Nexus proof failed; inspect retained driver/proof logs")
        state["status"] = "passed"
        print("Nexus protocol, security controls and offline client proof passed.", flush=True)
    except Exception:
        state["status"] = "failed"
        raise
    finally:
        initial.unlink(missing_ok=True)
        if container_id is not None:
            info = json.loads(run(runtime, ["inspect", container_id]).stdout)[0]
            if info["Config"]["Labels"].get(LABEL) == suffix:
                logs = subprocess.run([runtime, "logs", container_id], capture_output=True, timeout=30)
                (args.evidence / "nexus.stdout").write_bytes(logs.stdout)
                (args.evidence / "nexus.stderr").write_bytes(logs.stderr)
                run(runtime, ["stop", "--time", "60", container_id])
                run(runtime, ["rm", container_id])
                state["container_removed"] = True
        if volume is not None:
            info = json.loads(run(runtime, ["volume", "inspect", volume]).stdout)[0]
            if info.get("Labels", {}).get(LABEL) == suffix:
                run(runtime, ["volume", "rm", volume])
                state["volume_removed"] = True
        (args.evidence / "runner.json").write_text(json.dumps(state, indent=2) + "\n")


if __name__ == "__main__":
    main()
