#!/usr/bin/env python3
"""Release gate of the standard Trusted publishing pipeline (stdlib only).

On a `v*` tag push every check must pass, or the gate fails before anything is
built. On pull requests and manual runs it reports what it found, sets
publish=false and lets the dry run continue.

    release-gate.py --event EVENT --ref-name NAME --sha COMMIT --repo OWNER/REPO

GitHub API checks use GH_TOKEN (or GITHUB_TOKEN) when set. Outputs `version`,
`publish` and `crates` (JSON list in dependency order) are appended to
$GITHUB_OUTPUT when it is set.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import urllib.error
import urllib.request

UA = {'User-Agent': 'rustcommons-release-gate'}


def get_json(url: str, github: bool = False):
    headers = dict(UA)
    token = os.environ.get('GH_TOKEN') or os.environ.get('GITHUB_TOKEN')
    if github:
        headers['Accept'] = 'application/vnd.github+json'
        if token:
            headers['Authorization'] = f'Bearer {token}'
    request = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def workspace(root: Path):
    out = subprocess.check_output(
        ['cargo', 'metadata', '--no-deps', '--locked', '--format-version', '1',
         '--manifest-path', str(root / 'Cargo.toml')])
    data = json.loads(out)
    members = set(data['workspace_members'])
    return [p for p in data['packages'] if p['id'] in members]


def publishable(package) -> bool:
    return package.get('publish') is None or package['publish'] != []


def dependency_order(packages):
    """Release crates ordered so path dependencies publish first."""
    names = {p['name'] for p in packages}
    deps = {p['name']: {d['name'] for d in p['dependencies']
                        if d['name'] in names and d.get('path')} for p in packages}
    ordered: list[str] = []
    while deps:
        ready = sorted(n for n, d in deps.items() if d <= set(ordered))
        if not ready:
            raise ValueError(f'dependency cycle among release crates: {sorted(deps)}')
        ordered += ready
        for n in ready:
            del deps[n]
    return ordered


def checks(root: Path, args) -> tuple[str, list[str], list[str]]:
    """Return (version, crates, failures)."""
    failures: list[str] = []
    packages = [p for p in workspace(root) if publishable(p)]
    if not packages:
        raise ValueError('no publishable workspace crate')
    root_manifest = (root / 'Cargo.toml').resolve()
    root_pkg = next((p for p in packages if Path(p['manifest_path']).resolve() == root_manifest), None)
    version = args.ref_name[1:] if args.event == 'push' else (root_pkg or packages[0])['version']

    release = [p for p in packages if p['version'] == version]
    if not release:
        failures.append(f'no workspace crate has version {version}')
    for p in packages:
        if p['version'] != version:
            try:
                get_json(f"https://crates.io/api/v1/crates/{p['name']}/{p['version']}")
            except urllib.error.HTTPError as error:
                failures.append(f"{p['name']} {p['version']} is not in the release set and not on crates.io "
                                f'(HTTP {error.code})')
    crates = dependency_order(release)

    changelog = (root / 'CHANGELOG.md').read_text(encoding='utf-8') if (root / 'CHANGELOG.md').is_file() else ''
    entry = re.compile(r'^## \[' + re.escape(version) + r'\] - \d{4}-\d{2}-\d{2}\s*$', re.M)
    if not entry.search(changelog):
        failures.append(f'CHANGELOG.md has no dated "## [{version}] - YYYY-MM-DD" entry')
    notes = root / 'docs' / 'releases' / f'v{version}.md'
    if not notes.is_file():
        failures.append(f'missing release notes {notes.relative_to(root)}')
    docs = subprocess.run([sys.executable, str(root / 'scripts/check-release-docs.py'),
                           '--root', str(root), '--repo', args.repo],
                          capture_output=True, text=True)
    if docs.returncode != 0:
        failures.append('release documentation check: ' + (docs.stdout + docs.stderr).strip())

    if args.event == 'push':
        tag = args.ref_name
        if not re.fullmatch(r'v\d+\.\d+\.\d+', tag):
            failures.append(f'tag {tag} is not v plus a release version')
        if root_pkg is not None and tag != 'v' + root_pkg['version']:
            failures.append(f"tag {tag} does not match the root crate version {root_pkg['version']}")
        api = f'https://api.github.com/repos/{args.repo}'
        try:
            ref = get_json(f'{api}/git/ref/tags/{tag}', github=True)
            if ref['object']['type'] != 'tag':
                failures.append(f'{tag} is a lightweight tag; an annotated tag is required')
            else:
                target = get_json(ref['object']['url'], github=True)['object']
                if target['type'] != 'commit' or target['sha'] != args.sha:
                    failures.append(f"{tag} points to {target['type']} {target['sha']}, not {args.sha}")
        except urllib.error.HTTPError as error:
            failures.append(f'cannot read tag {tag} (HTTP {error.code})')
        try:
            compare = get_json(f'{api}/compare/{args.sha}...main', github=True)
            if compare['status'] not in ('ahead', 'identical'):
                failures.append(f"{args.sha} is not reachable from main (compare status {compare['status']})")
        except urllib.error.HTTPError as error:
            failures.append(f'cannot compare {args.sha} with main (HTTP {error.code})')
        try:
            # Filter the conclusion here: the API's status=success filter can omit
            # a completed successful run (observed for rust-mirror's v0.6.0 merge).
            runs = get_json(f'{api}/actions/workflows/ci.yml/runs?head_sha={args.sha}&per_page=100',
                            github=True)
            if not any(run.get('conclusion') == 'success' for run in runs['workflow_runs']):
                failures.append(f'no successful CI run for {args.sha}; wait for CI on main, then re-run')
        except urllib.error.HTTPError as error:
            failures.append(f'cannot read CI runs (HTTP {error.code})')
    return version, crates, failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--event', required=True)
    parser.add_argument('--ref-name', required=True)
    parser.add_argument('--sha', required=True)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--root', default='.')
    args = parser.parse_args()
    root = Path(args.root).resolve()
    publish = args.event == 'push' and args.ref_name.startswith('v')
    version, crates, failures = checks(root, args)

    print(f'Release gate: event={args.event} ref={args.ref_name} sha={args.sha}')
    print(f'Version {version}; release crates in order: {", ".join(crates) or "(none)"}')
    for failure in failures:
        print(f'- {failure}')
    if publish and failures:
        print('Release refused.', file=sys.stderr)
        return 1
    if not failures:
        print('All release checks passed.')
    if not publish:
        print('Dry run: publish=false.')
    output = os.environ.get('GITHUB_OUTPUT')
    if output:
        with open(output, 'a', encoding='utf-8') as f:
            f.write(f'version={version}\npublish={"true" if publish else "false"}\n'
                    f'crates={json.dumps(crates)}\n')
    return 0


if __name__ == '__main__':
    sys.exit(main())
