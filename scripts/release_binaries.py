#!/usr/bin/env python3
"""Package/validate tagged CLI binaries and append assets to an exact existing release.

Standard library only. Build jobs have read-only credentials; only the publish job
may attest/upload. No API in this helper deletes, replaces, or creates a release.
"""
from __future__ import annotations
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import struct
import tarfile
import tempfile
import time
import tomllib
import urllib.error
import urllib.request
import zipfile

REPO = 'rustcommons/rust-mirror'
TOOLCHAIN = '1.90.0'
TARGETS = ('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu',
           'aarch64-apple-darwin', 'x86_64-apple-darwin')
TAG = re.compile(r'v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?')
MAX_ARCHIVE = 128 * 1024 * 1024


def require(ok, message):
    if not ok:
        raise ValueError(message)


def run(*args, cwd=None):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def asset_name(version, target):
    require(target in TARGETS, 'unsupported release target')
    return f'rust-mirror-{version}-{target}' + ('.zip' if 'windows' in target else '.tar.gz')


def record(root, tag):
    require(TAG.fullmatch(tag), 'expected a v-prefixed semantic version tag')
    commit = run('git', 'rev-parse', f'refs/tags/{tag}^{{commit}}', cwd=root)
    manifest = tomllib.loads(run('git', 'show', f'{commit}:Cargo.toml', cwd=root))
    require(manifest['package']['name'] == 'rust-mirror' and manifest['package']['version'] == tag[1:],
            'tag and Cargo package version disagree')
    require(manifest['package']['rust-version'] == '1.90', 'review the pinned release toolchain after an MSRV change')
    notes = subprocess.check_output(['git', 'show', f'{commit}:docs/releases/{tag}.md'], cwd=root).decode()
    license_bytes = subprocess.check_output(['git', 'show', f'{commit}:LICENSE'], cwd=root)
    return dict(tag=tag, version=tag[1:], commit=commit, notes=notes, license=license_bytes)


def remote_commit(root, tag):
    refs = run('git', 'ls-remote', 'origin', f'refs/tags/{tag}', f'refs/tags/{tag}^{{}}', cwd=root).splitlines()
    values = dict(line.split('\t')[::-1] for line in refs)
    return values.get(f'refs/tags/{tag}^{{}}', values.get(f'refs/tags/{tag}'))


def api(path, method='GET', data=None, content_type='application/json'):
    require(path.startswith('https://api.github.com/') or path.startswith('https://uploads.github.com/'), 'unexpected GitHub API origin')
    headers = {'Accept': 'application/vnd.github+json', 'User-Agent': 'rust-mirror-release-binaries'}
    token = os.environ.get('GH_TOKEN')
    if token:
        headers['Authorization'] = f'Bearer {token}'
    if data is not None:
        headers['Content-Type'] = content_type
    request = urllib.request.Request(path, headers=headers, data=data, method=method)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), type('NoRedirect', (urllib.request.HTTPRedirectHandler,), {'redirect_request': lambda *a, **kw: None})())
    with opener.open(request, timeout=60) as response:
        return json.load(response)


def check_release(root, release_record, wait=0, allow_existing=False):
    deadline = time.monotonic() + wait
    while True:
        try:
            release = api(f'https://api.github.com/repos/{REPO}/releases/tags/{release_record["tag"]}')
            break
        except urllib.error.HTTPError as error:
            if error.code != 404 or time.monotonic() >= deadline:
                raise ValueError(f'existing GitHub Release unavailable (HTTP {error.code})') from None
            print('Waiting for the separately created GitHub Release', flush=True)
            time.sleep(min(10, max(0, deadline - time.monotonic())))
    validate_release(release, release_record, remote_commit(root, release_record['tag']))
    # Refuse all existing assets, including partial prior uploads. No automatic repair/delete.
    require(allow_existing or not release['assets'], 'release already has assets; refusing any replacement or partial rerun')
    return release


def validate_release(release, release_record, live_commit):
    require(live_commit == release_record['commit'], 'live tag moved from the frozen build commit')
    require(release['tag_name'] == release_record['tag'] and not release['draft'], 'release tag/draft mismatch')
    require(release['html_url'] == f'https://github.com/{REPO}/releases/tag/{release_record["tag"]}', 'release repository mismatch')
    require((release.get('body') or '').replace('\r\n', '\n') == release_record['notes'].replace('\r\n', '\n'), 'release body differs from tagged repository notes')


def smoke(binary, version):
    human = run(str(binary), '--version')
    require(human == f'rust-mirror {version}', 'binary --version mismatch')
    events = [json.loads(line) for line in run(str(binary), '--events', 'jsonl', '--version').splitlines()]
    require(events[0]['type'] == 'hello' and events[0]['protocol'] == 1 and events[0]['tool_version'] == version, 'binary hello mismatch')
    require(events[-1]['type'] == 'result' and events[-1]['ok'] and events[-1]['report']['tool_version'] == version, 'binary result mismatch')
    return human


def read_archive(path, prefix, executable):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= MAX_ARCHIVE, 'archive missing, linked or oversized')
    expected = {f'{prefix}/{name}' for name in (executable, 'LICENSE', 'README.txt')}
    entries = {}
    if path.suffix == '.zip':
        with zipfile.ZipFile(path) as archive:
            for info in archive.infolist():
                require(info.filename in expected and info.filename not in entries and info.file_size <= MAX_ARCHIVE, 'unexpected, duplicate or oversized ZIP member')
                require(not info.is_dir() and ((info.external_attr >> 16) & 0o170000) in (0, 0o100000), 'ZIP links/directories are forbidden')
                entries[info.filename] = archive.read(info)
    else:
        with tarfile.open(path) as archive:
            for info in archive:
                require(info.name in expected and info.name not in entries and info.isfile() and info.size <= MAX_ARCHIVE, 'unexpected, duplicate, linked or oversized tar member')
                require(info.mode == (0o755 if info.name.endswith('/' + executable) else 0o644), 'incorrect archive mode')
                entries[info.name] = archive.extractfile(info).read()
    require(set(entries) == expected and sum(map(len, entries.values())) <= MAX_ARCHIVE, 'incomplete or oversized archive')
    return {name.split('/', 1)[1]: content for name, content in entries.items()}


def windows_imports(data):
    """Read bounded PE32+ import names; record actual runtime DLL prerequisites."""
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    require(data[pe:pe + 4] == b'PE\0\0', 'Windows executable is not PE')
    require(struct.unpack_from('<H', data, pe + 4)[0] == 0x8664, 'Windows executable is not x86-64')
    count = struct.unpack_from('<H', data, pe + 6)[0]
    optional = pe + 24; size = struct.unpack_from('<H', data, pe + 20)[0]
    require(struct.unpack_from('<H', data, optional)[0] == 0x20b and count <= 96, 'unsupported PE layout')
    sections = []
    for n in range(count):
        virtual_size, address, raw_size, pointer = struct.unpack_from('<IIII', data, optional + size + 40 * n + 8)
        sections.append((address, max(virtual_size, raw_size), pointer))
    def offset(rva):
        for address, length, pointer in sections:
            if address <= rva < address + length:
                return pointer + rva - address
        raise ValueError('PE import address outside sections')
    start = offset(struct.unpack_from('<I', data, optional + 120)[0])
    names = []
    for n in range(128):
        row = struct.unpack_from('<IIIII', data, start + 20 * n)
        if not any(row): return names
        pos = offset(row[3]); end = data.find(b'\0', pos, pos + 256)
        require(end != -1, 'unterminated PE import name')
        name = data[pos:end].decode('ascii')
        require(re.fullmatch(r'[A-Za-z0-9_.-]+', name), 'unexpected PE import name')
        names.append(name)
    raise ValueError('PE import count exceeds bound')


def package(args):
    root = args.source.resolve()
    require(run('git', 'rev-parse', 'HEAD', cwd=root) == args.commit, 'source checkout differs from frozen commit')
    require(not run('git', 'status', '--porcelain', cwd=root), 'source checkout is dirty')
    r = record(root, args.tag)
    require(r['commit'] == args.commit, 'tag differs from frozen commit')
    compiler = run('rustc', '-vV')
    require(compiler.splitlines()[0].startswith('rustc ' + TOOLCHAIN + ' '), 'wrong Rust compiler')
    require(f'host: {args.target}' in compiler.splitlines(), 'native runner host differs from requested target')
    exe = 'rust-mirror.exe' if 'windows' in args.target else 'rust-mirror'
    binary = root / 'target' / args.target / 'release' / exe
    human = smoke(binary, r['version'])
    dlls = windows_imports(binary.read_bytes()) if 'windows' in args.target else None
    glibc = None
    if args.target == 'x86_64-unknown-linux-gnu':
        symbols = run('readelf', '--version-info', str(binary))
        versions = {tuple(map(int, v.split('.'))) for v in re.findall(r'GLIBC_([0-9]+\.[0-9]+(?:\.[0-9]+)?)', symbols)}
        require(versions and max(versions) <= (2, 35), 'binary requires newer glibc than Ubuntu 22.04')
        glibc = {'runner': run('getconf', 'GNU_LIBC_VERSION'), 'maximum_required_symbol': '.'.join(map(str, max(versions))), 'tested_floor': '2.35', 'ldd': run('ldd', str(binary))}
        require(glibc['runner'] == 'glibc 2.35', 'review changed Ubuntu 22.04 glibc baseline')
    prefix = f'rust-mirror-{r["version"]}-{args.target}'
    readme = f'''rust-mirror {r['version']} ({args.target})
Prepare verified Rust toolchain and curated crate mirrors for offline development.
Source: https://github.com/{REPO}/tree/{r['tag']}
Source commit: {r['commit']}
Built with Rust {TOOLCHAIN}; run ./{exe} --version (Windows: .\\{exe} --version).
Put the executable in a folder on PATH; no Rust/Cargo installation is needed.
Verify SHA256SUMS and provenance before running:
  gh attestation verify {asset_name(r['version'], args.target)} --repo {REPO}
Instructions: https://github.com/{REPO}/blob/main/docs/INSTALL.md
Unsigned executable: no Windows Authenticode or Apple Developer ID signing/notarization.
Windows may show SmartScreen: verify first, then More info > Run anyway.
macOS may show Gatekeeper: verify first, then Privacy & Security > Open Anyway.
Do not disable platform protection globally. GitHub provenance is separate from OS signing.
License: Apache-2.0; see LICENSE.
'''
    if glibc:
        readme += f"Linux: tested on Ubuntu 22.04/glibc 2.35; maximum required GLIBC symbol {glibc['maximum_required_symbol']}.\nOther distributions/older glibc are not qualified by this runner smoke test.\n"
        readme += 'Dynamic dependencies: ' + ', '.join(line.split()[0] for line in glibc['ldd'].splitlines()) + '\n'
    if dlls:
        readme += 'Windows imported DLLs: ' + ', '.join(dlls) + '\n'
        readme += 'If VCRUNTIME140.dll is missing, install the official Microsoft Visual C++ v14 x64 Redistributable.\nhttps://learn.microsoft.com/cpp/windows/latest-supported-vc-redist\n'
    files = {exe: binary.read_bytes(), 'LICENSE': r['license'], 'README.txt': readme.encode()}
    require(sum(map(len, files.values())) <= MAX_ARCHIVE, 'binary archive payload too large')
    args.output.mkdir(parents=True, exist_ok=False)
    destination = args.output / asset_name(r['version'], args.target)
    timestamp = int(run('git', 'show', '-s', '--format=%ct', args.commit, cwd=root))
    if destination.suffix == '.zip':
        with zipfile.ZipFile(destination, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
            for name, data in files.items():
                info = zipfile.ZipInfo(prefix + '/' + name, time.gmtime(max(timestamp, 315532800))[:6])
                info.external_attr = (0o100755 if name == exe else 0o100644) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(info, data)
    else:
        import gzip
        with destination.open('wb') as file, gzip.GzipFile(filename='', mode='wb', fileobj=file, mtime=timestamp) as gz, tarfile.open(fileobj=gz, mode='w') as archive:
            for name, data in files.items():
                info = tarfile.TarInfo(prefix + '/' + name); info.size = len(data)
                info.mode = 0o755 if name == exe else 0o644; info.mtime = timestamp
                archive.addfile(info, io.BytesIO(data))
    contents = read_archive(destination, prefix, exe)
    require(contents == files, 'archive does not reproduce the built files')
    with tempfile.TemporaryDirectory(prefix='rust-mirror-binary-smoke-') as directory:
        extracted = Path(directory) / exe; extracted.write_bytes(contents[exe]); extracted.chmod(0o755)
        smoke(extracted, r['version'])
    report = {'tag': r['tag'], 'commit': r['commit'], 'target': args.target, 'archive': destination.name, 'sha256': digest(destination.read_bytes()), 'binary_sha256': digest(files[exe]), 'license_sha256': digest(r['license']), 'rustc': compiler, 'smoke': human, 'archive_smoke': True, 'glibc': glibc, 'windows_imports': dlls}
    (args.output / 'build-report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


def collect(root, r, artifacts, output):
    output.mkdir(parents=True, exist_ok=False)
    for target in TARGETS:
        folder = artifacts / f'binary-{target}'
        report = json.loads((folder / 'build-report.json').read_text())
        name = asset_name(r['version'], target); path = folder / name
        require(report['target'] == target and report['commit'] == r['commit'] and report['tag'] == r['tag'] and report['archive'] == name and report['archive_smoke'] is True, 'build report does not match frozen tag/target')
        raw = path.read_bytes(); require(digest(raw) == report['sha256'], 'archive digest differs from build report')
        exe = 'rust-mirror.exe' if 'windows' in target else 'rust-mirror'
        files = read_archive(path, name.removesuffix('.zip').removesuffix('.tar.gz'), exe)
        require(files['LICENSE'] == r['license'] and digest(files[exe]) == report['binary_sha256'], 'license/binary does not match the tagged build')
        require(f"Source commit: {r['commit']}\n".encode() in files['README.txt'], 'archive source record mismatch')
        (output / name).write_bytes(raw)
    sums = ''.join(f'{digest(p.read_bytes())}  {p.name}\n' for p in sorted(output.iterdir()))
    (output / 'SHA256SUMS').write_text(sums)
    return sums


def main():
    p = argparse.ArgumentParser(description=__doc__); sub = p.add_subparsers(dest='command', required=True)
    resolve = sub.add_parser('resolve'); resolve.add_argument('--root', type=Path, required=True); resolve.add_argument('--tag', required=True); resolve.add_argument('--output', type=Path, required=True)
    build = sub.add_parser('package'); build.add_argument('--source', type=Path, required=True); build.add_argument('--tag', required=True); build.add_argument('--commit', required=True); build.add_argument('--target', choices=TARGETS, required=True); build.add_argument('--output', type=Path, required=True)
    for command in ('collect', 'preflight', 'upload'):
        parser = sub.add_parser(command); parser.add_argument('--root', type=Path, required=True); parser.add_argument('--tag', required=True); parser.add_argument('--commit', required=True)
        if command == 'collect': parser.add_argument('--artifacts', type=Path, required=True)
        if command in ('collect', 'upload'): parser.add_argument('--output', type=Path, required=True)
        if command == 'preflight':
            parser.add_argument('--wait-seconds', type=int, default=0)
            parser.add_argument('--allow-existing', action='store_true', help='Read-only dry-run only; upload always refuses existing assets')
    args = p.parse_args()
    if args.command == 'package': return package(args)
    r = record(args.root, args.tag)
    if args.command == 'resolve':
        require(remote_commit(args.root, args.tag) == r['commit'], 'local and live tag differ')
        with args.output.open('a') as out:
            for key in ('tag', 'version', 'commit'): out.write(f'{key}={r[key]}\n')
        print(json.dumps({k: r[k] for k in ('tag', 'version', 'commit')})); return
    require(r['commit'] == args.commit, 'tag changed since build preparation')
    if args.command == 'collect': print(collect(args.root, r, args.artifacts, args.output)); return
    release = check_release(args.root, r, args.wait_seconds if args.command == 'preflight' else 0, args.allow_existing if args.command == 'preflight' else False)
    if args.command == 'preflight': print('Existing release/tag/body match; dry-run allows existing assets' if args.allow_existing else 'Existing release/tag/body match; no assets present'); return
    expected = sorted([asset_name(r['version'], t) for t in TARGETS] + ['SHA256SUMS'])
    require(sorted(p.name for p in args.output.iterdir()) == expected, 'unexpected or missing release asset')
    sums = ''.join(f'{digest((args.output / n).read_bytes())}  {n}\n' for n in expected if n != 'SHA256SUMS')
    require((args.output / 'SHA256SUMS').read_text() == sums, 'checksum manifest mismatch')
    # GitHub rejects duplicate names (422). No clobber, PATCH or DELETE path exists.
    for name in expected:
        raw = (args.output / name).read_bytes()
        result = api(f'https://uploads.github.com/repos/{REPO}/releases/{release["id"]}/assets?name={name}', 'POST', raw, 'application/octet-stream')
        require(result['name'] == name and result['size'] == len(raw), 'uploaded asset mismatch')
        require(result.get('digest') == 'sha256:' + digest(raw), 'GitHub upload digest mismatch')
        print('Uploaded:', name, digest(raw), flush=True)


if __name__ == '__main__':
    try: main()
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError, struct.error) as error: raise SystemExit(str(error))
