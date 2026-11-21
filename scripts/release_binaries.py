#!/usr/bin/env python3
"""Package/validate tagged CLI binaries, the crate and the release profile.

Standard library only. build-binaries.yml builds and signs; trusted-publishing.yml's
`github-release` job uses `release` here, which creates a draft, checks its state,
uploads only missing assets and then publishes. No path deletes or replaces an asset.
"""
from __future__ import annotations
import argparse
import base64
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
import urllib.request
import zipfile

REPO = 'rustcommons/rust-mirror'
TOOLCHAIN = '1.90.0'
TARGETS = ('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu', 'x86_64-unknown-linux-musl',
           'aarch64-apple-darwin', 'x86_64-apple-darwin')
TAG = re.compile(r'v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?')
MAX_ARCHIVE = 128 * 1024 * 1024
MAX_PROVENANCE = 4 * 1024 * 1024
SLSA_PROVENANCE = 'https://slsa.dev/provenance/v1'
FIRST_PROVENANCE_ASSET = (0, 5, 5)


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


def provenance_name(version):
    return f'rust-mirror-{version}.intoto.jsonl'


def has_provenance_asset(version):
    """Releases up to v0.5.4 are immutable and carry provenance only in GitHub's store."""
    return tuple(int(n) for n in re.match(r'([0-9]+)\.([0-9]+)\.([0-9]+)', version).groups()) >= FIRST_PROVENANCE_ASSET


def provenance_bundles(raw):
    """Parse Sigstore bundles (one JSON document or JSON Lines) and their SLSA statements."""
    require(0 < len(raw) <= MAX_PROVENANCE, 'empty or oversized provenance bundle')
    text = raw.decode('utf-8')
    try:
        bundles = [json.loads(text)]
    except json.JSONDecodeError:
        bundles = [json.loads(line) for line in text.splitlines() if line.strip()]
    result = []
    for bundle in bundles:
        require(isinstance(bundle, dict) and str(bundle.get('mediaType', '')).startswith('application/vnd.dev.sigstore.bundle'), 'not a Sigstore bundle')
        require(isinstance(bundle.get('verificationMaterial'), dict), 'bundle lacks verification material')
        envelope = bundle.get('dsseEnvelope') or {}
        require(envelope.get('payloadType') == 'application/vnd.in-toto+json' and envelope.get('signatures'), 'bundle lacks a signed in-toto DSSE envelope')
        statement = json.loads(base64.b64decode(envelope['payload'], validate=True))
        require(statement.get('_type') == 'https://in-toto.io/Statement/v1' and statement.get('predicateType') == SLSA_PROVENANCE, 'bundle is not SLSA v1 provenance')
        result.append((bundle, statement))
    require(bool(result), 'no provenance bundles')
    return result


def envelope_key(bundle):
    envelope = bundle['dsseEnvelope']
    return envelope['payload'], tuple(sorted(s['sig'] for s in envelope['signatures']))


def provenance_jsonl(raw, subjects):
    """Return canonical JSON Lines whose provenance subjects are exactly the release assets."""
    covered = {}
    lines = []
    for bundle, statement in provenance_bundles(raw):
        for subject in statement['subject']:
            sha = subject['digest']['sha256']
            require(covered.setdefault(subject['name'], sha) == sha, 'conflicting provenance subject digests')
        lines.append(json.dumps(bundle, separators=(',', ':')) + '\n')
    require(covered == subjects, 'provenance subjects differ from the release assets')
    return ''.join(lines)


def verify_attached_provenance(assets, provenance, policy, scratch):
    """Verify every asset with the attached bundle and match it to GitHub's stored attestations."""
    attached = {envelope_key(bundle) for bundle, _ in provenance_bundles(provenance.read_bytes())}
    matched = set()
    for asset in sorted(assets):
        subprocess.run(['gh', 'attestation', 'verify', str(asset), '--bundle', str(provenance), *policy, '--predicate-type', SLSA_PROVENANCE], check=True, stdout=subprocess.DEVNULL)
        folder = scratch / asset.name; folder.mkdir(parents=True)
        subprocess.run(['gh', 'attestation', 'download', str(asset.resolve()), '--repo', REPO, '--predicate-type', SLSA_PROVENANCE], check=True, cwd=folder, stdout=subprocess.DEVNULL)
        # gh names the file sha256:DIGEST.jsonl, or sha256-DIGEST.jsonl on Windows (':' is an NTFS stream separator).
        downloads = sorted(folder.glob('*.jsonl'))
        require(bool(downloads), f'no GitHub-stored provenance downloaded for {asset.name}')
        stored = {envelope_key(bundle) for path in downloads for bundle, _ in provenance_bundles(path.read_bytes())}
        require(bool(attached & stored), f'attached provenance does not match GitHub-stored attestations for {asset.name}')
        matched |= attached & stored
    require(matched == attached, 'attached provenance contains bundles absent from GitHub attestation storage')


def crate_name(version):
    return f'rust-mirror-{version}.crate'


def has_single_profile(version):
    """Releases after 0.6.0 come from trusted-publishing.yml with one SHA256SUMS that also lists the .crate."""
    return tuple(int(n) for n in re.match(r'([0-9]+)\.([0-9]+)\.([0-9]+)', version).groups()) > (0, 6, 0)


def build_summed_assets(version):
    """Files every new build checksums: the .crate, all five archives and the SBOM."""
    return sorted([asset_name(version, t) for t in TARGETS] + [sbom_name(version), crate_name(version)])


def published_profile(version, live_names):
    """Exact asset layout of a published release. v0.5.1 to v0.6.0 keep their original profiles."""
    targets = legacy_or_current_targets(version, live_names)
    summed = [asset_name(version, t) for t in targets]
    if not has_legacy_assets(version):
        summed.append(sbom_name(version))
    if has_single_profile(version):
        summed.append(crate_name(version))
    summed = sorted(summed)
    bundle = provenance_name(version) if has_provenance_asset(version) else None
    return dict(summed=summed, signed=summed + ['SHA256SUMS'], bundle=bundle,
                assets=set(summed) | {'SHA256SUMS'} | ({bundle} if bundle else set()))


def audit_crate(source, commit, crate):
    """The packaged crate holds only regular files equal to the clean frozen commit."""
    prefix = crate.name.removesuffix('.crate') + '/'
    require(crate.is_file() and not crate.is_symlink() and crate.stat().st_size <= MAX_ARCHIVE, 'missing, linked or oversized crate')
    with tarfile.open(crate, 'r:gz') as archive:
        members = archive.getmembers()
        require(bool(members) and all(m.isfile() and m.name.startswith(prefix) for m in members), 'crate must hold only regular files under one root')
        names = [m.name[len(prefix):] for m in members]
        require(not any(n.startswith(('fuzz/', '.clusterfuzzlite/', '.github/')) for n in names), 'fuzz or CI content packaged')
        for member, name in zip(members, names):
            if name in ('Cargo.toml', '.cargo_vcs_info.json'):
                continue
            original = subprocess.check_output(['git', 'show', f"{commit}:{'Cargo.toml' if name == 'Cargo.toml.orig' else name}"], cwd=source)
            require(archive.extractfile(member).read() == original, f'{name} differs from the frozen commit')
        vcs = json.loads(archive.extractfile(prefix + '.cargo_vcs_info.json').read())
    require(vcs['git']['sha1'] == commit and not vcs['git'].get('dirty', False), 'crate VCS record is not the clean frozen commit')
    return len(members)


def crate_vcs_commit(crate):
    prefix = crate.name.removesuffix('.crate') + '/'
    with tarfile.open(crate, 'r:gz') as archive:
        vcs = json.loads(archive.extractfile(prefix + '.cargo_vcs_info.json').read())
    return vcs['git']['sha1'], bool(vcs['git'].get('dirty', False))


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


def api(url, method='GET', data=None, content_type='application/json'):
    headers = {'Accept': 'application/vnd.github+json', 'User-Agent': 'rust-mirror-release'}
    token = os.environ.get('GH_TOKEN') or os.environ.get('GITHUB_TOKEN')
    if token:
        headers['Authorization'] = f'Bearer {token}'
    if data is not None:
        headers['Content-Type'] = content_type
    request = urllib.request.Request(url, headers=headers, data=data, method=method)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), type('NoRedirect', (urllib.request.HTTPRedirectHandler,), {'redirect_request': lambda *a, **kw: None})())
    with opener.open(request, timeout=120) as response:
        return json.load(response)


def notes_title(notes):
    title = next((line[2:].strip() for line in notes.splitlines() if line.startswith('# ')), '')
    require(bool(title), 'release notes have no first-level heading')
    return title


def check_release_state(release, r, title, draft):
    """Refuse a release that is not exactly ours: tag, title, body, draft and prerelease state."""
    require(release['tag_name'] == r['tag'], 'release is on a different tag')
    require(release['name'] == title, 'release title differs from the committed notes')
    require((release.get('body') or '').replace('\r\n', '\n') == r['notes'].replace('\r\n', '\n'), 'release body differs from the committed notes')
    require(release['draft'] is draft and release['prerelease'] is False, 'release draft/prerelease state is not as expected')
    if not draft:
        require(release['html_url'] == f'https://github.com/{REPO}/releases/tag/{r["tag"]}', 'release repository mismatch')


def publish_release(r, folder, call=None):
    """Create or resume the draft release, upload only missing assets, check them, then publish.

    A killed run leaves a draft (the tags API returns 404 for drafts), so the draft is
    found through the releases list. Existing assets must equal the local bytes; none is
    ever replaced or deleted. A published release must already be complete and is not modified.
    """
    call = call or api
    base = f'https://api.github.com/repos/{REPO}'
    files = {p.name: p for p in sorted(folder.iterdir())}
    want = {name: 'sha256:' + digest(path.read_bytes()) for name, path in files.items()}
    title = notes_title(r['notes'])
    matches, page = [], 1
    while True:
        batch = call(f'{base}/releases?per_page=100&page={page}')
        matches += [x for x in batch if x['tag_name'] == r['tag']]
        if len(batch) < 100:
            break
        page += 1
    require(len(matches) <= 1, 'several releases exist for the tag')
    if matches:
        release = matches[0]
    else:
        release = call(f'{base}/releases', 'POST', json.dumps({'tag_name': r['tag'], 'name': title, 'body': r['notes'], 'draft': True, 'prerelease': False}).encode())
        print('Created draft release', release['id'], flush=True)
    check_release_state(release, r, title, release['draft'])
    existing = {a['name']: a for a in release['assets']}
    require(set(existing) <= set(want), 'release holds an unexpected asset')
    for name, asset in existing.items():
        require(asset.get('digest') == want[name] and asset['state'] == 'uploaded', f'existing asset {name} differs; never replacing it')
    if not release['draft']:
        require(set(existing) == set(want), 'published release is incomplete; refusing to modify a published release')
        print('Release already published with every asset', flush=True)
        return release
    for name in sorted(set(want) - set(existing)):
        raw = files[name].read_bytes()
        result = call(f'https://uploads.github.com/repos/{REPO}/releases/{release["id"]}/assets?name={name}', 'POST', raw, 'application/octet-stream')
        require(result['name'] == name and result['size'] == len(raw) and result.get('digest') == want[name], 'uploaded asset mismatch')
        print('Uploaded:', name, want[name], flush=True)
    release = call(f'{base}/releases/{release["id"]}')
    check_release_state(release, r, title, True)
    require({a['name']: a.get('digest') for a in release['assets']} == want and all(a['state'] == 'uploaded' for a in release['assets']), 'draft assets incomplete before publishing')
    call(f'{base}/releases/{release["id"]}', 'PATCH', json.dumps({'draft': False, 'make_latest': 'true'}).encode())
    release = call(f'{base}/releases/tags/{r["tag"]}')
    check_release_state(release, r, title, False)
    require({a['name']: a.get('digest') for a in release['assets']} == want, 'published assets differ')
    print('Published', r['tag'], 'with', len(want), 'assets', flush=True)
    return release


def verify_sbom_bindings(paths, sbom, policy):
    """Each archive and the crate carries an SBOM attestation whose predicate is this exact SBOM."""
    from release_sbom import validate_binding
    document = json.loads(sbom.read_text(encoding='utf-8'))
    for path in paths:
        proof = subprocess.check_output(['gh', 'attestation', 'verify', str(path), *policy, '--predicate-type', 'https://cyclonedx.org/bom', '--format', 'json'], text=True, encoding='utf-8')
        validate_binding(json.loads(proof), document, digest(path.read_bytes()))


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


def collect(root, r, artifacts, output, sbom=None, artifact_prefix="", crate=None):
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
    require(crate is not None and crate.name == crate_name(r['version']) and not crate.is_symlink() and crate.stat().st_size <= MAX_ARCHIVE, 'missing, misnamed, linked or oversized crate')
    require(crate_vcs_commit(crate) == (r['commit'], False), 'crate VCS record differs from the frozen build')
    (output / crate.name).write_bytes(crate.read_bytes())
    require(sorted(p.name for p in output.iterdir()) == build_summed_assets(r['version']), 'collected files differ from the release profile')
    sums = ''.join(f'{digest(p.read_bytes())}  {p.name}\n' for p in sorted(output.iterdir()))
    (output / 'SHA256SUMS').write_text(sums)
    return sums


def staged_assets(r, output, provenance=None):
    """Check the exact upload set: SHA256SUMS covers the .crate, archives and SBOM; the bundle covers those and SHA256SUMS."""
    summed = build_summed_assets(r['version'])
    signed = summed + ['SHA256SUMS']
    bundle = provenance_name(r['version'])
    if provenance is not None:
        require(not (output / bundle).exists(), 'provenance asset already staged')
        subjects = {n: digest((output / n).read_bytes()) for n in signed}
        # Binary write: LF-only JSON Lines on every platform (no text-mode CRLF translation).
        (output / bundle).write_bytes(provenance_jsonl(provenance.read_bytes(), subjects).encode('utf-8'))
    expected = sorted(signed + [bundle])
    require(sorted(p.name for p in output.iterdir()) == expected, 'unexpected or missing release asset')
    sums = ''.join(f'{digest((output / n).read_bytes())}  {n}\n' for n in summed)
    require((output / 'SHA256SUMS').read_text() == sums, 'checksum manifest mismatch')
    provenance_jsonl((output / bundle).read_bytes(), {n: digest((output / n).read_bytes()) for n in signed})
    return expected


def main():
    p = argparse.ArgumentParser(description=__doc__); sub = p.add_subparsers(dest='command', required=True)
    resolve = sub.add_parser('resolve'); resolve.add_argument('--root', type=Path, required=True); resolve.add_argument('--tag', required=True); resolve.add_argument('--output', type=Path, required=True); resolve.add_argument('--candidate-commit', help='Read-only pre-tag qualification of this exact commit')
    build = sub.add_parser('package'); build.add_argument('--source', type=Path, required=True); build.add_argument('--tag', required=True); build.add_argument('--commit', required=True); build.add_argument('--target', choices=TARGETS, required=True); build.add_argument('--output', type=Path, required=True); build.add_argument('--glibc-report', type=Path); build.add_argument('--candidate', action='store_true', help='Package a frozen pre-tag candidate; cannot authorize upload')
    stage = sub.add_parser('stage', help='Offline: add the provenance bundle and check the exact upload set')
    stage.add_argument('--root', type=Path, required=True); stage.add_argument('--tag', required=True); stage.add_argument('--commit', required=True)
    stage.add_argument('--output', type=Path, required=True); stage.add_argument('--provenance', type=Path, required=True)
    stage.add_argument('--candidate', action='store_true', help='Read-only pre-tag candidate validation')
    audit = sub.add_parser('audit-crate', help='Offline: compare the packaged crate with the clean frozen commit')
    audit.add_argument('--source', type=Path, required=True); audit.add_argument('--commit', required=True); audit.add_argument('--crate', type=Path, required=True)
    publish_parser = sub.add_parser('release', help='Check provenance and SBOM bindings, then create/resume the draft release and publish it')
    publish_parser.add_argument('--root', type=Path, required=True); publish_parser.add_argument('--tag', required=True); publish_parser.add_argument('--commit', required=True)
    publish_parser.add_argument('--assets', type=Path, required=True)
    collect_parser = sub.add_parser('collect'); collect_parser.add_argument('--root', type=Path, required=True); collect_parser.add_argument('--tag', required=True); collect_parser.add_argument('--commit', required=True)
    collect_parser.add_argument('--artifacts', type=Path, required=True)
    collect_parser.add_argument('--candidate', action='store_true', help='Read-only pre-tag candidate validation')
    collect_parser.add_argument('--sbom', type=Path, required=True)
    collect_parser.add_argument('--artifact-prefix', default='')
    collect_parser.add_argument('--crate', type=Path, required=True)
    collect_parser.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    if args.command == 'package': return package(args)
    if args.command == 'audit-crate': print(f'Crate audit passed: {audit_crate(args.source, args.commit, args.crate)} regular members, VCS {args.commit}.'); return
    candidate_commit = args.candidate_commit if args.command == 'resolve' else args.commit if args.command in ('collect', 'stage') and args.candidate else None
    r = record(args.root, args.tag, candidate_commit)
    if args.command == 'resolve':
        if candidate_commit is None:
            require(remote_commit(args.root, args.tag) == r['commit'], 'local and live tag differ')
        with args.output.open('a') as out:
            for key in ('tag', 'version', 'commit'): out.write(f'{key}={r[key]}\n')
        print(json.dumps({k: r[k] for k in ('tag', 'version', 'commit')})); return
    require(r['commit'] == args.commit, 'tag changed since build preparation')
    if args.command == 'release':
        require(remote_commit(args.root, args.tag) == r['commit'], 'live tag moved')
        names = sorted(p.name for p in args.assets.iterdir())
        require(names == sorted(build_summed_assets(r['version']) + ['SHA256SUMS', provenance_name(r['version'])]), 'unexpected or missing release asset')
        policy = attestation_policy(r)
        bundle = args.assets / provenance_name(r['version'])
        verify_attached_provenance([args.assets / n for n in names if n != bundle.name], bundle, policy, Path(tempfile.mkdtemp(prefix='rust-mirror-store-')))
        verify_sbom_bindings([args.assets / n for n in names if n.endswith(('.zip', '.tar.gz', '.crate'))], args.assets / sbom_name(r['version']), policy)
        publish_release(r, args.assets)
        return
    if args.command == 'collect': print(collect(args.root, r, args.artifacts, args.output, args.sbom, args.artifact_prefix, args.crate)); return
    print('\n'.join(staged_assets(r, args.output, args.provenance)))

if __name__ == '__main__':
    try: main()
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError, struct.error) as error: raise SystemExit(str(error))
