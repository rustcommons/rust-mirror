#!/usr/bin/env python3
"""Generate a bounded all-target Cargo dependency SBOM from an exact clean commit."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile
import tomllib


def validate(document, name, version):
    if document.get('bomFormat') != 'CycloneDX' or document.get('specVersion') != '1.5':
        raise ValueError('expected CycloneDX 1.5 JSON')
    root = document.get('metadata', {}).get('component', {})
    if (root.get('name'), root.get('version')) != (name, version):
        raise ValueError('SBOM root differs from the frozen package')
    components = document.get('components', [])
    if not components or len(components) > 10000:
        raise ValueError('empty or oversized dependency SBOM')
    if any(not c.get('name') or not c.get('version') for c in components):
        raise ValueError('SBOM has incomplete dependency identities')


def validate_binding(verified_proofs, document, archive_sha256):
    for proof in verified_proofs:
        statement = proof.get('verificationResult', {}).get('statement', {})
        if statement.get('predicateType') == 'https://cyclonedx.org/bom' and statement.get('predicate') == document:
            if any(s.get('digest', {}).get('sha256') == archive_sha256 for s in statement.get('subject', [])):
                return
    raise ValueError('verified archive attestation does not bind this exact SBOM')


def canonicalize(document, scratch, commit, lock_sha256):
    # Cargo's local package IDs contain the disposable source directory. Rewrite
    # only that exact prefix in IDs/references together, preserving the graph.
    old = 'path+file://' + str(scratch)
    new = 'git+https://github.com/rustcommons/rust-mirror@' + commit
    def visit(value):
        if isinstance(value, str):
            return value.replace(old, new).replace('?download_url=file://.', '')
        if isinstance(value, list):
            return [visit(v) for v in value]
        if isinstance(value, dict):
            return {k: visit(v) for k, v in value.items()}
        return value
    result = visit(document)
    result['metadata'].setdefault('properties', []).extend([
        {'name': 'rust-mirror:source:commit', 'value': commit},
        {'name': 'rust-mirror:source:lock-sha256', 'value': lock_sha256},
        {'name': 'rust-mirror:scope', 'value': 'All-target Cargo dependency graph; excludes OS runtimes and CI tools'},
    ])
    return result


def generate(source, commit, tool, output):
    source = source.resolve()
    actual = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source, text=True).strip()
    if actual != commit or subprocess.check_output(['git', 'status', '--porcelain'], cwd=source):
        raise ValueError('SBOM source must be the clean frozen commit')
    if subprocess.check_output([str(tool), 'cyclonedx', '--version'], text=True).strip() not in ('cargo-cyclonedx 0.5.9', 'cargo-cyclonedx-cyclonedx 0.5.9'):
        raise ValueError('wrong pinned SBOM generator')
    raw = subprocess.check_output(['git', 'archive', '--format=tar', commit], cwd=source)
    if len(raw) > 64 * 1024 * 1024:
        raise ValueError('SBOM source archive exceeds 64 MiB')
    epoch = subprocess.check_output(['git', 'show', '-s', '--format=%ct', commit], cwd=source, text=True).strip()
    with tempfile.TemporaryDirectory(prefix='rust-mirror-sbom-') as temp:
        scratch = Path(temp)
        with tarfile.open(fileobj=io.BytesIO(raw)) as archive:
            for member in archive.getmembers():
                path = PurePosixPath(member.name)
                if path.is_absolute() or '..' in path.parts or not (member.isfile() or member.isdir()):
                    raise ValueError('unsafe frozen SBOM source member')
                destination = scratch.joinpath(*path.parts)
                if member.isdir():
                    destination.mkdir(parents=True, exist_ok=True)
                else:
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    with destination.open('xb') as file:
                        file.write(archive.extractfile(member).read())
        manifest = tomllib.loads((scratch / 'Cargo.toml').read_text())['package']
        lock = (scratch / 'Cargo.lock').read_bytes()
        env = dict(os.environ, SOURCE_DATE_EPOCH=epoch, CARGO_BUILD_JOBS='2')
        subprocess.run(['cargo', 'metadata', '--locked', '--all-features', '--format-version', '1'], cwd=scratch, env=env, check=True, stdout=subprocess.DEVNULL)
        subprocess.run([str(tool), 'cyclonedx', '--format', 'json', '--all-features', '--target', 'all', '--spec-version', '1.5', '--override-filename', 'release.cdx'], cwd=scratch, env=env, check=True)
        if (scratch / 'Cargo.lock').read_bytes() != lock:
            raise ValueError('SBOM generation changed the frozen dependency lock')
        data = (scratch / 'release.cdx.json').read_bytes()
        if len(data) > 8 * 1024 * 1024:
            raise ValueError('SBOM exceeds 8 MiB')
        document = json.loads(data)
        validate(document, manifest['name'], manifest['version'])
        expected = {(p['name'], p['version']): p['checksum'] for p in tomllib.loads(lock.decode())['package'] if p.get('source', '').startswith('registry+')}
        observed = {(p['name'], p['version']): next(h['content'] for h in p.get('hashes', []) if h['alg'] == 'SHA-256') for p in document['components']}
        if expected != observed:
            raise ValueError('SBOM dependency checksums differ from the frozen full lockfile')
        document = canonicalize(document, scratch, commit, hashlib.sha256(lock).hexdigest())
        data = (json.dumps(document, sort_keys=True, indent=2) + '\n').encode()
        with output.open('xb') as file:
            file.write(data)
        print(json.dumps({'source': commit, 'generator': 'cargo-cyclonedx 0.5.9', 'lock_sha256': hashlib.sha256(lock).hexdigest(), 'sbom_sha256': hashlib.sha256(data).hexdigest(), 'target': 'all', 'all_features': True, 'source_date_epoch': epoch}))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source', type=Path, required=True)
    p.add_argument('--commit', required=True)
    p.add_argument('--tool', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    generate(a.source, a.commit, a.tool.resolve(), a.output)


if __name__ == '__main__':
    main()
