#!/usr/bin/env python3
"""Read-only crates.io checksum and exact-tag payload verification for one crate."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile
import time
import tomllib
import urllib.error
import urllib.request
import release_binaries as release


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def get(url):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    request = urllib.request.Request(url, headers={'User-Agent': 'rust-mirror-release-verification (https://github.com/rustcommons/rust-mirror)'})
    with opener.open(request, timeout=45) as response:
        data = response.read(16 * 1024 * 1024 + 1)
    if len(data) > 16 * 1024 * 1024:
        raise ValueError('registry response too large')
    return data


def verify_payload(raw, root, record):
    with tarfile.open(fileobj=io.BytesIO(raw)) as archive:
        members = archive.getmembers()
        if len(members) > 1000 or sum(m.size for m in members) > 32 * 1024 * 1024:
            raise ValueError('registry payload exceeds size/count bounds')
        prefix = 'rust-mirror-' + record['version'] + '/'
        if not all(m.isfile() and m.name.startswith(prefix) and m.size <= 8 * 1024 * 1024 for m in members):
            raise ValueError('invalid registry archive members')
        files = {m.name[len(prefix):]: archive.extractfile(m).read() for m in members}
        if len(files) != len(members):
            raise ValueError('duplicate registry archive members')
    original = tomllib.loads(files['Cargo.toml.orig'].decode())
    normalized = tomllib.loads(files['Cargo.toml'].decode())
    for key, value in original['package'].items():
        if key in ('name', 'version', 'edition', 'rust-version', 'license', 'repository', 'readme') and normalized['package'].get(key) != value:
            raise ValueError('normalized registry manifest differs: ' + key)
    for section in ('dependencies', 'build-dependencies', 'dev-dependencies'):
        expected = {k: {'version': v} if isinstance(v, str) else v for k, v in original.get(section, {}).items()}
        if normalized.get(section, {}) != expected:
            raise ValueError('normalized registry dependencies differ: ' + section)
    if normalized.get('features', {}) != original.get('features', {}) or normalized.get('target', {}) != original.get('target', {}):
        raise ValueError('normalized registry feature/target policy differs')
    vcs = json.loads(files['.cargo_vcs_info.json'])
    if vcs['git']['sha1'] != record['commit'] or vcs['git'].get('dirty', False):
        raise ValueError('registry VCS differs from clean release tag')
    allowed = {p for p in release.run('git', 'ls-tree', '-r', '--name-only', record['commit'], cwd=root).splitlines()
               if p.startswith(('src/', 'examples/')) or p in ('Cargo.toml', 'Cargo.lock', 'README.md', 'CHANGELOG.md', 'LICENSE')}
    if set(files) != allowed | {'Cargo.toml.orig', '.cargo_vcs_info.json'}:
        raise ValueError('unexpected registry payload paths')
    for path in allowed:
        archive_path = 'Cargo.toml.orig' if path == 'Cargo.toml' else path
        if files[archive_path] != subprocess.check_output(['git', 'show', record['commit'] + ':' + path], cwd=root):
            raise ValueError('registry payload differs from release tag: ' + path)
    return vcs


def check(root, tag, wait=0, allow_missing=False):
    record = release.record(root, tag)
    deadline = time.monotonic() + wait
    while True:
        try:
            metadata = json.loads(get('https://crates.io/api/v1/crates/rust-mirror/' + record['version']))['version']
            break
        except urllib.error.HTTPError as error:
            if error.code != 404:
                raise
            if time.monotonic() >= deadline:
                if allow_missing:
                    return {'version': record['version'], 'source': record['commit'], 'published': False}
                raise ValueError('published registry version not available') from None
            time.sleep(min(5, max(0, deadline - time.monotonic())))
    raw = get('https://static.crates.io/crates/rust-mirror/rust-mirror-' + record['version'] + '.crate')
    digest = hashlib.sha256(raw).hexdigest()
    if metadata['checksum'] != digest or metadata['yanked']:
        raise ValueError('registry checksum/yank mismatch')
    index = next(json.loads(line) for line in get('https://index.crates.io/ru/st/rust-mirror').decode().splitlines() if json.loads(line)['vers'] == record['version'])
    if index['cksum'] != digest or index.get('yanked'):
        raise ValueError('sparse-index checksum/yank mismatch')
    vcs = verify_payload(raw, root, record)
    return {'version': record['version'], 'source': record['commit'], 'published': True, 'archive_sha256': digest, 'vcs': vcs}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, default=Path('.'))
    p.add_argument('--tag', required=True)
    p.add_argument('--wait-seconds', type=int, default=0)
    p.add_argument('--allow-missing', action='store_true')
    p.add_argument('--github-output', type=Path)
    a = p.parse_args()
    result = check(a.root, a.tag, a.wait_seconds, a.allow_missing)
    print(json.dumps(result, indent=2))
    if a.github_output:
        with a.github_output.open('a') as out:
            out.write('published=' + str(result['published']).lower() + '\n')


if __name__ == '__main__':
    main()
