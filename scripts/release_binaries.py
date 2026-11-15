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
import platform
import zlib
from pathlib import Path
import re
import subprocess
import struct
import shutil
import tarfile
import tempfile
import time
import tomllib
import urllib.error
import urllib.request
import zipfile

REPO = 'rustcommons/rust-mirror'
TOOLCHAIN = '1.90.0'
TARGETS = ('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu', 'x86_64-unknown-linux-musl',
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


def sbom_name(version):
    return f'rust-mirror-{version}.cdx.json'


def has_legacy_assets(version):
    return version in ('0.5.1', '0.5.2')



def attestation_policy(expected):
    """Bind future attestations to the approved source and reusable signer commit."""
    legacy = has_legacy_assets(expected['version'])
    signer = 'release-binaries.yml' if legacy else 'build-binaries.yml'
    flags = ['--repo', REPO, '--signer-workflow', REPO + '/.github/workflows/' + signer]
    if not legacy:
        require(re.fullmatch(r'[0-9a-f]{40}', expected['commit']), 'expected exact attestation source commit')
        flags += ['--source-digest', expected['commit'], '--signer-digest', expected['commit'], '--deny-self-hosted-runners']
    return flags


def record(root, tag, candidate_commit=None):
    require(TAG.fullmatch(tag), 'expected a v-prefixed semantic version tag')
    if candidate_commit is not None:
        require(re.fullmatch(r'[0-9a-f]{40}', candidate_commit), 'candidate needs an exact commit ID')
        commit = run('git', 'rev-parse', '--verify', candidate_commit + '^{commit}', cwd=root)
        require(commit == candidate_commit, 'candidate commit differs from resolved object')
    else:
        commit = run('git', 'rev-parse', f'refs/tags/{tag}^{{commit}}', cwd=root)
    manifest = tomllib.loads(run('git', 'show', f'{commit}:Cargo.toml', cwd=root))
    require(manifest['package']['name'] == 'rust-mirror' and manifest['package']['version'] == tag[1:],
            'tag and Cargo package version disagree')
    require(manifest['package']['rust-version'] == '1.90', 'review the pinned release toolchain after an MSRV change')
    notes = subprocess.check_output(['git', 'show', f'{commit}:docs/releases/{tag}.md'], cwd=root).decode()
    license_bytes = subprocess.check_output(['git', 'show', f'{commit}:LICENSE'], cwd=root)
    lock_sha256 = digest(subprocess.check_output(['git', 'show', f'{commit}:Cargo.lock'], cwd=root))
    return dict(tag=tag, version=tag[1:], commit=commit, notes=notes, license=license_bytes, lock_sha256=lock_sha256)


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


def validate_windows_imports(names):
    require(bool(names), 'Windows DLL import audit parsed no imports')
    forbidden = [name for name in names if re.match(r'^(vcruntime|msvcp|ucrtbase|api-ms-win-crt-)', name, re.I)]
    require(not forbidden, 'Windows release still requires external C runtime DLLs: ' + ', '.join(forbidden))


def dumpbin_imports(binary):
    """Use the runner's installed MSVC inspection tool; no SDK installation or settings change."""
    vswhere = Path(os.environ['ProgramFiles(x86)']) / 'Microsoft Visual Studio/Installer/vswhere.exe'
    install = run(str(vswhere), '-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath')
    candidates = sorted((Path(install) / 'VC/Tools/MSVC').glob('*/bin/Hostx64/x64/dumpbin.exe'))
    require(bool(candidates), 'MSVC dumpbin unavailable on Windows release runner')
    output = run(str(candidates[-1]), '/dependents', str(binary))
    names = re.findall(r'^\s+([A-Za-z0-9_.-]+\.dll)\s*$', output, re.I | re.M)
    validate_windows_imports(names)
    return names, output


def validate_static_linux(headers, dynamic, file_output, ldd_output):
    require('INTERP' not in headers and '(NEEDED)' not in dynamic, 'Linux binary has a dynamic interpreter or shared-library dependency')
    require('x86-64' in file_output and ('static-pie linked' in file_output or 'statically linked' in file_output), 'file does not identify a static x86-64 executable')
    require('not a dynamic executable' in ldd_output or 'statically linked' in ldd_output, 'ldd does not confirm static linkage')


def validate_dynamic_linux(headers, dynamic, versions, file_output):
    require('INTERP' in headers and 'x86-64' in file_output, 'GNU binary must be dynamic x86-64')
    needed = re.findall(r'\(NEEDED\).*?\[(.*?)\]', dynamic)
    require({'libc.so.6', 'libgcc_s.so.1'} <= set(needed), 'GNU binary lacks expected dynamic runtimes')
    names = sorted(set(re.findall(r'\bGLIBC_([0-9]+(?:\.[0-9]+)+)\b', versions)), key=lambda s: tuple(map(int, s.split('.'))))
    require(bool(names) and tuple(map(int, names[-1].split('.'))) <= (2, 17), 'GNU binary exceeds GLIBC_2.17 or lacks symbol evidence')
    return dict(maximum_required_glibc=names[-1], required_glibc_versions=names, needed=needed)


def static_linux(binary, version):
    headers = run('readelf', '--program-headers', str(binary))
    dynamic = run('readelf', '--dynamic', str(binary))
    description = run('file', str(binary))
    ldd = subprocess.run(['ldd', str(binary)], capture_output=True, text=True)
    output = (ldd.stdout + ldd.stderr).strip()
    validate_static_linux(headers, dynamic, description, output)
    # An empty root has no dynamic loader, libc, OpenSSL, NSS modules or other libraries.
    with tempfile.TemporaryDirectory(prefix='rust-mirror-static-root-') as directory:
        executable = Path(directory) / 'rust-mirror'
        shutil.copyfile(binary, executable); executable.chmod(0o755)
        actual = run('sudo', 'chroot', directory, '/rust-mirror', '--version')
        require(actual == f'rust-mirror {version}', 'empty-root static executable smoke failed')
    return {'file': description, 'ldd': output, 'ldd_exit': ldd.returncode, 'program_headers': headers, 'dynamic_section': dynamic, 'no_interpreter_or_needed': True, 'empty_root_version': actual}


def legacy_or_current_targets(version, asset_names):
    """v0.5.1's four archives are immutable; later releases have both Linux variants."""
    if version == '0.5.1':
        return tuple(t for t in TARGETS if t != 'x86_64-unknown-linux-gnu')
    return TARGETS


def validate_gnu_report(report, commit, binary_hash):
    require(report['method'] == 'cargo-libc' and report['target_glibc'] == '2.17', 'wrong GNU publication method/target')
    require(report['cargo_libc_version'] == 'cargo-libc 0.7.0' and report['cargo_libc_commit'] == 'e68ca7bf3a8ffd973c8576acc48abd0fbcc66d25', 'GNU producer revision differs from qualification')
    require(report['source_commit'] == commit and report['binary_sha256'] == binary_hash, 'GNU report differs from the built source/binary')
    maximum = report['linkage']['maximum_required_glibc']
    require(maximum is not None and tuple(map(int, maximum.split('.'))) <= (2, 17), 'GNU artifact exceeds qualified glibc target')
    require({'libc.so.6', 'libgcc_s.so.1'} <= set(report['linkage']['needed']), 'GNU artifact must use the dynamic runtime profile')
    gates = report['runtime_gates']
    required = {
        'almalinux:8@sha256:2d0f2eb2a83cc09dbc2eb92fade4dbe8f709628149e81aa00aec70630406603b',
        'centos:7@sha256:be65f488b7764ad3638f236b7b515b3678369a5124c47b8d32916d6487418ea4',
    }
    require(required <= {row['image'] for row in gates}, 'GNU artifact missing required CentOS7/Alma8 gates')
    require(all(row['result'] == 'passed' and row['version_ok'] and row['plan_ok'] and row['sync_ok'] for row in gates), 'GNU runtime launch/plan/sync gate failed')


def package(args):
    root = args.source.resolve()
    require(run('git', 'rev-parse', 'HEAD', cwd=root) == args.commit, 'source checkout differs from frozen commit')
    require(not run('git', 'status', '--porcelain', cwd=root), 'source checkout is dirty')
    r = record(root, args.tag, args.commit if args.candidate else None)
    require(r['commit'] == args.commit, 'tag differs from frozen commit')
    compiler = run('rustc', '-vV')
    require(compiler.splitlines()[0].startswith('rustc ' + TOOLCHAIN + ' '), 'wrong Rust compiler')
    host = 'x86_64-unknown-linux-gnu' if args.target == 'x86_64-unknown-linux-musl' else args.target
    require(f'host: {host}' in compiler.splitlines(), 'native runner architecture differs from requested target')
    exe = 'rust-mirror.exe' if 'windows' in args.target else 'rust-mirror'
    binary = root / 'target' / args.target / 'release' / exe
    human = smoke(binary, r['version'])
    dlls = windows_imports(binary.read_bytes()) if 'windows' in args.target else None
    dumpbin = None
    if dlls is not None:
        validate_windows_imports(dlls)
        inspected, dumpbin = dumpbin_imports(binary)
        require({n.lower() for n in dlls} == {n.lower() for n in inspected}, 'PE and dumpbin import audits disagree')
    glibc = None
    if args.target == 'x86_64-unknown-linux-gnu':
        require(args.glibc_report is not None, 'GNU archive requires qualified cargo-libc build report')
        glibc = json.loads(args.glibc_report.read_text())
        validate_gnu_report(glibc, args.commit, digest(binary.read_bytes()))
    linux_static = static_linux(binary, r['version']) if args.target == 'x86_64-unknown-linux-musl' else None
    prefix = f'rust-mirror-{r["version"]}-{args.target}'
    source_ref = r['commit'] if args.candidate else r['tag']
    candidate_note = 'UNRELEASED CANDIDATE: not a public release; no production attestations.\n' if args.candidate else ''
    readme = f'''{candidate_note}rust-mirror {r['version']} ({args.target})
Prepare verified Rust toolchain and curated crate mirrors for offline development.
Source: https://github.com/{REPO}/tree/{source_ref}
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
    if linux_static:
        readme += 'Any x86_64 Linux: fully static musl executable; no glibc or shared-library runtime dependency.\n'
        readme += 'DNS uses /etc/hosts and /etc/resolv.conf; glibc NSS plugins are not loaded.\n'
        readme += 'TLS uses rustls and bundled public roots; add corporate CAs explicitly with --extra-ca-file.\n'
        readme += 'Kernel, CPU and network policy still apply; named smoke tests are not every enterprise configuration.\n'
    if glibc:
        readme += 'Dynamic GNU/Linux variant: cargo-libc0.7.0 targets glibc2.17.\n'
        readme += 'Launch + real HTTPS plan/sync gates: CentOS7, AlmaLinux8, Leap42.3, Ubuntu16/18/20, Debian8/9.\n'
        readme += 'Container userlands only; old native kernels were not tested. Use musl for glibc below2.17.\n'
        readme += 'Maximum required GLIBC symbol: ' + str(glibc['linkage']['maximum_required_glibc']) + '\n'
        readme += 'Runtime libraries: ' + ', '.join(glibc['linkage']['needed']) + '\n'
        readme += 'Prefer the static musl archive when avoiding glibc/libgcc runtime dependencies.\n'
    if dlls:
        readme += 'Windows imported DLLs: ' + ', '.join(dlls) + '\n'
        readme += 'Nothing else to install: the C runtime is built in, so no Visual C++ Redistributable is needed.\n'
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
    report = {'runner_image_version': os.environ.get('ImageVersion'), 'python_version': platform.python_version(), 'zlib_version': zlib.ZLIB_VERSION, 'host_os': platform.system(), 'tag': r['tag'], 'commit': r['commit'], 'target': args.target, 'archive': destination.name, 'sha256': digest(destination.read_bytes()), 'binary_sha256': digest(files[exe]), 'license_sha256': digest(r['license']), 'rustc': compiler, 'smoke': human, 'archive_smoke': True, 'linux_static': linux_static, 'glibc_baseline': glibc, 'windows_imports': dlls, 'dumpbin_dependents': dumpbin, 'windows_crt_static': dlls is not None}
    (args.output / 'build-report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


def collect(root, r, artifacts, output, sbom=None, artifact_prefix=""):
    require(re.fullmatch(r'(?:[a-z][a-z0-9-]{0,20})?', artifact_prefix), 'unsafe artifact prefix')
    output.mkdir(parents=True, exist_ok=False)
    for target in TARGETS:
        folder = artifacts / f'{artifact_prefix}binary-{target}'
        report = json.loads((folder / 'build-report.json').read_text())
        name = asset_name(r['version'], target); path = folder / name
        require(report['target'] == target and report['commit'] == r['commit'] and report['tag'] == r['tag'] and report['archive'] == name and report['archive_smoke'] is True, 'build report does not match frozen tag/target')
        raw = path.read_bytes(); require(digest(raw) == report['sha256'], 'archive digest differs from build report')
        exe = 'rust-mirror.exe' if 'windows' in target else 'rust-mirror'
        files = read_archive(path, name.removesuffix('.zip').removesuffix('.tar.gz'), exe)
        require(files['LICENSE'] == r['license'] and digest(files[exe]) == report['binary_sha256'], 'license/binary does not match the tagged build')
        require(f"Source commit: {r['commit']}\n".encode() in files['README.txt'], 'archive source record mismatch')
        (output / name).write_bytes(raw)
    require(sbom is not None and sbom.is_file() and not sbom.is_symlink() and sbom.stat().st_size <= 8 * 1024 * 1024, 'missing, linked or oversized SBOM')
    from release_sbom import validate
    sbom_data = sbom.read_bytes()
    validate(json.loads(sbom_data), 'rust-mirror', r['version'])
    properties = {p['name']: p['value'] for p in json.loads(sbom_data)['metadata'].get('properties', [])}
    require(properties.get('rust-mirror:source:commit') == r['commit'] and properties.get('rust-mirror:source:lock-sha256') == r['lock_sha256'], 'SBOM source/lock differs from frozen build')
    (output / sbom_name(r['version'])).write_bytes(sbom_data)
    sums = ''.join(f'{digest(p.read_bytes())}  {p.name}\n' for p in sorted(output.iterdir()))
    (output / 'SHA256SUMS').write_text(sums)
    return sums


def main():
    p = argparse.ArgumentParser(description=__doc__); sub = p.add_subparsers(dest='command', required=True)
    resolve = sub.add_parser('resolve'); resolve.add_argument('--root', type=Path, required=True); resolve.add_argument('--tag', required=True); resolve.add_argument('--output', type=Path, required=True); resolve.add_argument('--candidate-commit', help='Read-only pre-tag qualification of this exact commit')
    build = sub.add_parser('package'); build.add_argument('--source', type=Path, required=True); build.add_argument('--tag', required=True); build.add_argument('--commit', required=True); build.add_argument('--target', choices=TARGETS, required=True); build.add_argument('--output', type=Path, required=True); build.add_argument('--glibc-report', type=Path); build.add_argument('--candidate', action='store_true', help='Package a frozen pre-tag candidate; cannot authorize upload')
    for command in ('collect', 'preflight', 'upload'):
        parser = sub.add_parser(command); parser.add_argument('--root', type=Path, required=True); parser.add_argument('--tag', required=True); parser.add_argument('--commit', required=True)
        if command == 'collect':
            parser.add_argument('--artifacts', type=Path, required=True)
            parser.add_argument('--candidate', action='store_true', help='Read-only pre-tag candidate validation')
            parser.add_argument('--sbom', type=Path, required=True)
            parser.add_argument('--artifact-prefix', default='')
        if command in ('collect', 'upload'): parser.add_argument('--output', type=Path, required=True)
        if command == 'preflight':
            parser.add_argument('--wait-seconds', type=int, default=0)
            parser.add_argument('--allow-existing', action='store_true', help='Read-only dry-run only; upload always refuses existing assets')
    args = p.parse_args()
    if args.command == 'package': return package(args)
    candidate_commit = args.candidate_commit if args.command == 'resolve' else args.commit if args.command == 'collect' and args.candidate else None
    r = record(args.root, args.tag, candidate_commit)
    if args.command == 'resolve':
        if candidate_commit is None:
            require(remote_commit(args.root, args.tag) == r['commit'], 'local and live tag differ')
        with args.output.open('a') as out:
            for key in ('tag', 'version', 'commit'): out.write(f'{key}={r[key]}\n')
        print(json.dumps({k: r[k] for k in ('tag', 'version', 'commit')})); return
    require(r['commit'] == args.commit, 'tag changed since build preparation')
    if args.command == 'collect': print(collect(args.root, r, args.artifacts, args.output, args.sbom, args.artifact_prefix)); return
    release = check_release(args.root, r, args.wait_seconds if args.command == 'preflight' else 0, args.allow_existing if args.command == 'preflight' else False)
    if args.command == 'preflight': print('Existing release/tag/body match; dry-run allows existing assets' if args.allow_existing else 'Existing release/tag/body match; no assets present'); return
    expected = sorted([asset_name(r['version'], t) for t in TARGETS] + [sbom_name(r['version']), 'SHA256SUMS'])
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
