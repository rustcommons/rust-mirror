#!/usr/bin/env python3
"""Qualify the real crate sync CLI and isolated Cargo HTTPS client on the current OS."""
import argparse
import json
from pathlib import Path
import subprocess
from prove_crates_https import cargo_paths, command


def invoke(binary, args, evidence, name):
    completed = command([binary, "--events", "jsonl", "crates", *args])
    (evidence / (name + ".jsonl")).write_text(completed.stdout)
    (evidence / (name + ".stderr")).write_text(completed.stderr)
    records = [json.loads(line) for line in completed.stdout.splitlines()]
    assert records[0]["type"] == "hello" and records[0]["protocol"] == 1
    assert records[-1]["type"] == "result" and records[-1]["ok"]
    assert sum(r["type"] == "result" for r in records) == 1
    assert [r["seq"] for r in records] == list(range(len(records)))
    return records[-1]["report"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    cargo, rustc = cargo_paths()
    host = next(line.split(": ", 1)[1] for line in command([rustc, "-vV"]).stdout.splitlines() if line.startswith("host: "))
    catalog = evidence / "catalog.toml"
    catalog.write_text('[crates]\nitoa="=1.0.15"\n[crate-policy]\nhosts=[' + json.dumps(host) + ']\ntargets=[' + json.dumps(host) + ']\n[crate-output]\nbase-url="https://localhost:18443/client-generation"\n[crate-network]\ncontact="https://github.com/rustcommons/rust-mirror/issues"\n')
    output = evidence / "run"
    preview = invoke(binary, ["sync", "--config", catalog, "--output", output], evidence, "sync-preview")
    assert preview["status"] == "needs_approval" and not (output / "mirror").exists()
    assert not (output / ".download").exists()
    plan_id = preview["preview"]["plan_id"]
    assert preview["plan_id"] == plan_id
    shown = invoke(binary, ["show", "--plan", output / "plan.json"], evidence, "show")
    assert shown == preview["preview"]
    completed = invoke(binary, ["sync", "--config", catalog, "--output", output, "--approve-plan", plan_id], evidence, "sync-approved")
    assert completed["status"] == "completed" and completed["report"]["plan_id"] == plan_id
    assert completed["plan_id"] == plan_id and completed["mirror"] == completed["report"]["mirror"]
    script = Path(__file__).with_name("prove_crates_https.py")
    result = command([__import__("sys").executable, script, "--mirror", output / "mirror", "--evidence", evidence / "https-client", "--cargo", cargo, "--rustc", rustc])
    (evidence / "client.stdout").write_text(result.stdout)
    print(result.stdout, end="", flush=True)


if __name__ == "__main__":
    main()
