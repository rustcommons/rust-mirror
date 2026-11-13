#!/usr/bin/env python3
"""Write a catalog from the dated reviewed ranking, without fetching a new ranking."""
import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--base-url", default="https://localhost:18443/generation")
    args = parser.parse_args()
    source = Path(__file__).resolve().parent.parent
    roots = json.loads((source / "tests/fixtures/crates-top100-20261113.json").read_text())
    assert len(roots) == 100 and len({r["name"] for r in roots}) == 100
    text = "[crates]\n" + "".join(f'{json.dumps(r["name"])} = {json.dumps("=" + r["version"])}\n' for r in roots)
    text += '\n[crate-policy]\nhosts=["x86_64-unknown-linux-gnu","x86_64-pc-windows-msvc"]\n'
    text += 'targets=["x86_64-unknown-linux-gnu","x86_64-pc-windows-msvc"]\n'
    text += '\n[crate-output]\nbase-url=' + json.dumps(args.base_url) + '\n'
    text += '\n[crate-network]\ncontact="https://github.com/rustcommons/rust-mirror/issues"\n'
    with args.output.open("x") as file:
        file.write(text)


if __name__ == "__main__":
    main()
