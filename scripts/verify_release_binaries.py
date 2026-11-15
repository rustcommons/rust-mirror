#!/usr/bin/env python3
"""Read-only verification of public release downloads; execute only after provenance checks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import re
import release_binaries as release


def command(args, env=None):
    return subprocess.check_output([str(a) for a in args], text=True, env=env)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--tag', required=True)
    p.add_argument('--target', choices=['x86_64-pc-windows-msvc', 'x86_64-unknown-linux-musl'], required=True)
    p.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    root = Path(__file__).resolve().parents[1]
    expected = release.record(root, args.tag)
    live = json.loads(command(['gh', 'api', f'repos/{release.REPO}/releases/tags/{args.tag}']))
    release.validate_release(live, expected, release.remote_commit(root, args.tag))
    targets = release.legacy_or_current_targets(expected['version'], [a['name'] for a in live['assets']])
    names = sorted(release.asset_name(expected['version'], target) for target in targets)
    assets = {a['name']: a for a in live['assets']}
    release.require(len(live['assets']) == len(names) + 1 and set(assets) == set(names + ['SHA256SUMS']), 'expected exact release archive profile and SHA256SUMS')
    release.require(all(a['state'] == 'uploaded' and a['size'] <= release.MAX_ARCHIVE for a in assets.values()), 'asset incomplete or oversized')
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    downloads = output / 'downloads'
    command(['gh', 'release', 'download', args.tag, '--repo', release.REPO, '--dir', downloads])
    lines = (downloads / 'SHA256SUMS').read_text().splitlines()
    release.require(len(lines) == len(names) and all(re.fullmatch(r'[0-9a-f]{64}  [A-Za-z0-9_.-]+', line) for line in lines), 'invalid checksum manifest')
    checksums = dict(line.split('  ')[::-1] for line in lines)
    release.require(len(checksums) == len(names) and list(checksums) == names, 'checksum names/order/duplicates mismatch')
    attestations = {}
    for name in sorted(assets):
        path = downloads / name; actual = release.digest(path.read_bytes())
        release.require(assets[name]['digest'] == 'sha256:' + actual, 'GitHub asset digest mismatch')
        if name != 'SHA256SUMS': release.require(checksums[name] == actual, 'archive checksum mismatch')
        proof = command(['gh', 'attestation', 'verify', path, '--repo', release.REPO, '--signer-workflow', release.REPO + '/.github/workflows/release-binaries.yml', '--format', 'json'])
        result = json.loads(proof); release.require(bool(result), 'empty attestation verification')
        (output / ('attestation-' + name + '.json')).write_text(proof)
        attestations[name] = dict(sha256=actual, verified=True)
    chosen = release.asset_name(expected['version'], args.target)
    exe = 'rust-mirror.exe' if 'windows' in args.target else 'rust-mirror'
    prefix = chosen.removesuffix('.zip').removesuffix('.tar.gz')
    contents = release.read_archive(downloads / chosen, prefix, exe)
    release.require(contents['LICENSE'] == expected['license'], 'license differs from source tag')
    release.require(f"Source commit: {expected['commit']}\n".encode() in contents['README.txt'], 'archive source record differs from tag')
    extracted = output / prefix; extracted.mkdir()
    for name, raw in contents.items(): (extracted / name).write_bytes(raw)
    binary = extracted / exe; binary.chmod(0o755)
    # Downloaded product code does not need workflow/API credentials for --version.
    public_env = {k: v for k, v in os.environ.items() if k not in ('GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'ACTIONS_ID_TOKEN_REQUEST_TOKEN')}
    human = command([binary, '--version'], env=public_env).strip()
    release.require(human == 'rust-mirror ' + expected['version'], 'downloaded binary version mismatch')
    events = [json.loads(line) for line in command([binary, '--events', 'jsonl', '--version'], env=public_env).splitlines()]
    release.require(events[0]['protocol'] == 1 and events[-1]['ok'] and events[-1]['report']['tool_version'] == expected['version'], 'downloaded binary events mismatch')
    linkage = {}
    if 'windows' in args.target:
        imports = release.windows_imports(binary.read_bytes()); release.validate_windows_imports(imports)
        linkage = {'imports': imports, 'external_crt_imports': False}
    else:
        headers = command(['readelf', '--program-headers', binary]); dynamic = command(['readelf', '--dynamic', binary])
        file_info = command(['file', binary]).strip()
        ldd = subprocess.run(['ldd', str(binary)], capture_output=True, text=True, env=public_env)
        ldd_text = (ldd.stdout + ldd.stderr).strip()
        release.validate_static_linux(headers, dynamic, file_info, ldd_text)
        linkage = dict(file=file_info, ldd=ldd_text, no_interpreter_or_needed=True)
    report = {'tag': args.tag, 'source': expected['commit'], 'release_url': live['html_url'], 'assets': attestations, 'checksums_match': True, 'notes_unchanged': True, 'target': args.target, 'downloaded_archive': chosen, 'binary_sha256': release.digest(binary.read_bytes()), 'human_version': human, 'events': events, 'linkage': linkage, 'read_only': True}
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__': main()
