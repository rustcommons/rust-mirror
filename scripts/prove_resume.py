#!/usr/bin/env python3
"""Bounded real interruption/resume proof using a previously verified selected snapshot.

Uses the official download origin. No installs, Nexus writes, global caches or
user toolchain homes. Kills only its own child; output/evidence must be new.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time
import tomllib
import publish_raw


def prove(cli, plan_dir, baseline, output, evidence):
    if output.exists():
        raise ValueError("proof output must be new")
    evidence.mkdir(parents=True, exist_ok=False)
    result = {"status": "running", "scope": "Actual official-origin interruption and resume, seeded from verified prior snapshot"}
    child = None
    try:
        document, files = publish_raw.local_files(baseline)
        plan = tomllib.loads((plan_dir / "plan.toml").read_text(encoding="utf-8"))
        if document.get("schema") != 2 or document["selection"] != plan["selection"]:
            raise ValueError("baseline must be the same selected snapshot")
        inventory = {n: (p, sha) for n, p, sha in files}
        artifacts = {a["sha256"]: a for a in plan["artifacts"]}
        for a in artifacts.values():
            if a["path"] not in inventory or inventory[a["path"]][1] != a["sha256"]:
                raise ValueError("baseline does not satisfy frozen payload plan")
        result["plan_sha256"] = hashlib.sha256((plan_dir / "plan.toml").read_bytes()).hexdigest()
        result["baseline_inventory_sha256"] = publish_raw.digest(baseline / "snapshot.toml")
        args = [str(cli), "fetch", "--plan", str(plan_dir), "--output", str(output)]
        with (evidence / "interrupted.stdout").open("xb") as stdout, (evidence / "interrupted.stderr").open("xb") as stderr:
            child = subprocess.Popen(args, stdout=stdout, stderr=stderr)
            deadline = time.monotonic() + 90
            interrupted = None
            while time.monotonic() < deadline and child.poll() is None:
                for part in (output / ".download/files").glob("*.part"):
                    key = part.stem
                    if key in artifacts:
                        total = inventory[artifacts[key]["path"]][0].stat().st_size
                        n = part.stat().st_size
                        if 65536 <= n < total:
                            interrupted = (part, key, total)
                            break
                if interrupted:
                    break
                time.sleep(0.01)
            if not interrupted:
                raise RuntimeError("did not observe a substantial incomplete transfer within the bound")
            child.kill()
            result["interrupted_exit"] = child.wait(timeout=30)
        part, key, total = interrupted
        prefix = part.stat().st_size
        if not 0 < prefix < total or (output / "mirror").exists():
            raise RuntimeError("transfer finished before interruption; cannot count as resume evidence")
        result["interrupted_artifact"] = artifacts[key]["path"]
        result["prefix_bytes"] = prefix
        result["artifact_bytes"] = total
        result["prefix_sha256"] = publish_raw.digest(part)
        seeded = []
        for sha, a in artifacts.items():
            if sha == key:
                continue
            target = output / ".download/files" / (sha + ".data")
            if target.is_symlink():
                raise ValueError("unexpected cache link")
            if not target.exists():
                shutil.copyfile(inventory[a["path"]][0], target)
                seeded.append(a["path"])
            if publish_raw.digest(target) != sha:
                raise ValueError("seeded bytes did not verify")
        result["seeded_from_baseline"] = seeded
        with (evidence / "resume.stdout").open("xb") as stdout, (evidence / "resume.stderr").open("xb") as stderr:
            completed = subprocess.run([*args, "--resume"], stdout=stdout, stderr=stderr, timeout=300)
        result["resume_exit"] = completed.returncode
        if completed.returncode:
            raise RuntimeError("resume failed; inspect retained logs")
        log = (evidence / "resume.stderr").read_text(encoding="utf-8")
        if "Resume: " + artifacts[key]["path"] not in log:
            raise RuntimeError("no resumed-tail event recorded")
        _, current = publish_raw.local_files(output / "mirror")
        if {n: h for n, p, h in current} != {n: h for n, p, h in files}:
            raise RuntimeError("completed bytes differ from qualified baseline")
        if any((output / ".download/files").iterdir()):
            raise RuntimeError("successful fetch retained payload cache")
        result["final_inventory_sha256"] = publish_raw.digest(output / "mirror/snapshot.toml")
        result["verified_files"] = len(current)
        result["status"] = "passed"
        result["limits"] = ["Seeded other files; not a second full network download", "No new Rustup installation or foreign-platform runtime qualification", "Trusted private local filesystem; no power-loss or hostile mutation claim", "Resume log indicates requested offset; wire-byte count is not measured by this driver"]
    finally:
        if child is not None and child.poll() is None:
            child.kill()
            child.wait(timeout=30)
        (evidence / "summary.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "plan", "baseline", "output", "evidence"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(prove(args.cli.resolve(), args.plan.resolve(), args.baseline.resolve(), args.output.resolve(), args.evidence.resolve()), indent=2))


if __name__ == "__main__":
    main()
