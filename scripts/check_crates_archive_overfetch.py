#!/usr/bin/env python3
"""Measure actual verified archive bytes against the same full-index Cargo lock union."""
import argparse
import hashlib
import json
from pathlib import Path
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mirror", type=Path, required=True)
    parser.add_argument("--conformance", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    marker = (args.mirror / "snapshot.toml").read_bytes()
    inventory = tomllib.loads(marker.decode())
    assert inventory["schema"] == 4 and inventory["kind"] == "crates"
    files = {f["path"]: f for f in inventory["files"]}
    versions = {(v["name"], v["version"]) for v in inventory["plan"]["versions"]}
    union = set()
    locks = list(args.conformance.glob("*-full/Cargo.lock"))
    expected = json.loads((args.conformance / "conformance.json").read_text())
    assert len(locks) == len(expected["roots"])
    for lock in locks:
        union.update((p["name"], p["version"]) for p in tomllib.loads(lock.read_text())["package"] if "source" in p)
    assert union and union <= versions, "Cargo union contains an unmirrored archive"
    lengths = {}
    for name, version in versions:
        path = f"cargo/crates/{name}/{name}-{version}.crate"
        file = files[path]
        local = args.mirror / path
        assert not local.is_symlink()
        with local.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        assert digest == file["sha256"] and local.stat().st_size == file["bytes"]
        lengths[name, version] = file["bytes"]
    native_bytes = sum(lengths.values())
    cargo_bytes = sum(lengths[identity] for identity in union)
    report = dict(plan_id=inventory["plan"]["plan_id"], inventory_sha256=hashlib.sha256(marker).hexdigest(),
                  cargo_roots=len(locks), native_versions=len(versions), cargo_union_versions=len(union),
                  native_archive_bytes=native_bytes, cargo_union_archive_bytes=cargo_bytes,
                  extra_archive_bytes=native_bytes-cargo_bytes,
                  archive_byte_overfetch_percent=100*(native_bytes-cargo_bytes)/cargo_bytes,
                  scope="Verified archive payload lengths; metadata, caches and filesystem blocks excluded")
    with args.output.open("x") as file:
        json.dump(report, file, indent=2)
        file.write("\n")
    print(json.dumps(report), flush=True)


if __name__ == "__main__":
    main()
