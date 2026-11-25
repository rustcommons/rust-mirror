"""Distribution safety controls: frozen source, archive boundaries and no-overwrite API use."""
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import struct
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
SPEC = importlib.util.spec_from_file_location('release_binaries', Path(__file__).resolve().parents[1] / 'scripts/release_binaries.py')
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class ReleaseBinariesTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='rust-mirror-release-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.record = dict(tag='v1.2.3', version='1.2.3', commit='a' * 40, notes='Reviewed notes\n', license=b'license', lock_sha256=release.digest(b'lock'))
        self.live = dict(tag_name='v1.2.3', draft=False, html_url='https://github.com/rustcommons/rust-mirror/releases/tag/v1.2.3', body='Reviewed notes\n', assets=[], id=42)

    def test_candidate_record_freezes_exact_commit_without_creating_a_tag(self):
        def git(*args):
            return subprocess.check_output(['git', *args], cwd=self.root, text=True).strip()
        git('init', '-q'); git('config', 'user.name', 'Release fixture'); git('config', 'user.email', 'fixture@example.invalid')
        (self.root / 'Cargo.toml').write_text('[package]\nname="rust-mirror"\nversion="1.2.3"\nrust-version="1.90"\n')
        (self.root / 'LICENSE').write_bytes(b'license')
        (self.root / 'Cargo.lock').write_bytes(b'lock')
        (self.root / 'docs/releases').mkdir(parents=True)
        (self.root / 'docs/releases/v1.2.3.md').write_text('Reviewed notes\n')
        git('add', '.'); git('commit', '-qm', 'Candidate source')
        commit = git('rev-parse', 'HEAD')
        with patch.object(release, 'remote_commit', side_effect=AssertionError('candidate must not need a public tag')):
            output = self.root / 'outputs'
            argv = ['helper', 'resolve', '--root', str(self.root), '--tag', 'v1.2.3', '--candidate-commit', commit, '--output', str(output)]
            with patch.object(sys, 'argv', argv): release.main()
        self.assertIn('commit=' + commit, output.read_text())
        self.assertEqual(git('tag'), '')
        self.assertEqual(release.record(self.root, 'v1.2.3', commit)['notes'], 'Reviewed notes\n')
        with self.assertRaisesRegex(ValueError, 'exact commit'):
            release.record(self.root, 'v1.2.3', 'HEAD')
        with self.assertRaisesRegex(ValueError, 'version disagree'):
            release.record(self.root, 'v1.2.4', commit)

    def test_retired_release_write_commands_no_longer_exist(self):
        # trusted-publishing.yml uploads; this helper has no release write path.
        for verb in ('upload', 'preflight'):
            with self.subTest(verb=verb), patch('sys.stderr', new=io.StringIO()):
                argv = ['helper', verb, '--root', str(self.root), '--tag', 'v1.2.3', '--commit', 'a'*40, '--output', str(self.root)]
                with patch.object(sys, 'argv', argv), self.assertRaises(SystemExit) as error:
                    release.main()
                self.assertEqual(error.exception.code, 2)
        self.assertFalse(hasattr(release, 'check_release'))

    def test_downloaded_gnu_abi_gate_refuses_newer_symbols_and_wrong_linkage(self):
        good = ('INTERP', '(NEEDED) Shared library: [libc.so.6]\n(NEEDED) Shared library: [libgcc_s.so.1]', 'GLIBC_2.2.5 GLIBC_2.17', 'ELF x86-64 dynamically linked')
        self.assertEqual(release.validate_dynamic_linux(*good)['maximum_required_glibc'], '2.17')
        for changed in (('', *good[1:]), (good[0], '', *good[2:]), (*good[:2], 'GLIBC_2.18', good[3]), (*good[:2], '', good[3]), (*good[:3], 'ELF ARM')):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                release.validate_dynamic_linux(*changed)

    def write_sbom(self, path, commit=None):
        document = {'bomFormat': 'CycloneDX', 'specVersion': '1.5',
                    'serialNumber': 'urn:uuid:12345678-1234-1234-1234-123456789012',
                    'metadata': {'component': {'name': 'rust-mirror', 'version': '1.2.3'},
                                 'properties': [{'name': 'rust-mirror:source:commit', 'value': commit or self.record['commit']},
                                                {'name': 'rust-mirror:source:lock-sha256', 'value': self.record['lock_sha256']}]},
                    'components': [{'name': 'fixture', 'version': '1.0.0'}]}
        path.write_text(json.dumps(document))

    def test_sbom_profile_does_not_change_published_legacy_releases(self):
        self.assertTrue(release.has_legacy_assets('0.5.1'))
        self.assertTrue(release.has_legacy_assets('0.5.2'))
        self.assertFalse(release.has_legacy_assets('0.5.3'))
        self.assertEqual(release.sbom_name('0.5.3'), 'rust-mirror-0.5.3.cdx.json')

    def test_source_record_uses_tag_not_changed_worktree(self):
        def git(*args):
            subprocess.run(['git', *args], cwd=self.root, check=True, capture_output=True)
        git('init', '-q')
        git('config', 'user.name', 'Release fixture')
        git('config', 'user.email', 'fixture@example.invalid')
        (self.root / 'Cargo.toml').write_text('[package]\nname="rust-mirror"\nversion="1.2.3"\nrust-version="1.90"\n')
        (self.root / 'LICENSE').write_bytes(b'license')
        (self.root / 'Cargo.lock').write_bytes(b'lock')
        (self.root / 'docs/releases').mkdir(parents=True)
        (self.root / 'docs/releases/v1.2.3.md').write_text('Reviewed notes\n')
        git('add', '.')
        git('commit', '-qm', 'Fixture source')
        git('tag', '-a', 'v1.2.3', '-m', 'Fixture')
        (self.root / 'Cargo.toml').write_text('dirty and unrelated')
        result = release.record(self.root, 'v1.2.3')
        self.assertEqual(result['notes'].replace('\r\n', '\n'), self.record['notes'])
        self.assertEqual(result['license'], b'license')
        git('tag', 'v1.2.4')
        with self.assertRaisesRegex(ValueError, 'version disagree'):
            release.record(self.root, 'v1.2.4')
        for bad in ('../main', 'v1.2.3\nname=bad', '--help', 'main'):
            with self.assertRaises(ValueError):
                release.record(self.root, bad)

    def test_release_refuses_body_tag_draft_repository_or_moved_commit(self):
        release.validate_release(self.live, self.record, self.record['commit'])
        for changed in (dict(body='unreviewed'), dict(tag_name='v9.9.9'), dict(draft=True), dict(html_url='https://example.invalid')):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                release.validate_release(dict(self.live, **changed), self.record, self.record['commit'])
        with self.assertRaisesRegex(ValueError, 'tag moved'):
            release.validate_release(self.live, self.record, 'b' * 40)

    def archive(self, suffix, bad=None):
        prefix = 'rust-mirror-1.2.3-x86_64-unknown-linux-gnu'
        names = ['rust-mirror', 'LICENSE', 'README.txt']
        path = self.root / ('fixture' + suffix)
        if suffix == '.zip':
            with zipfile.ZipFile(path, 'w') as archive:
                for name in names:
                    archive.writestr(prefix + '/' + name, b'file')
                if bad:
                    info = zipfile.ZipInfo(bad)
                    info.external_attr = 0o120777 << 16
                    archive.writestr(info, b'../../secret')
        else:
            with tarfile.open(path, 'w:gz') as archive:
                for name in names:
                    info = tarfile.TarInfo(prefix + '/' + name); info.size = 4
                    info.mode = 0o755 if name == 'rust-mirror' else 0o644
                    archive.addfile(info, io.BytesIO(b'file'))
                if bad:
                    info = tarfile.TarInfo(bad); info.type = tarfile.SYMTYPE; info.linkname = '../../secret'
                    archive.addfile(info)
        return path, prefix

    def test_pe_import_audit_records_runtime_and_refuses_wrong_machine(self):
        data = bytearray(1024)
        struct.pack_into('<I', data, 0x3c, 0x80)
        data[0x80:0x84] = b'PE\0\0'
        struct.pack_into('<HH', data, 0x84, 0x8664, 1)
        struct.pack_into('<H', data, 0x94, 240)
        struct.pack_into('<H', data, 0x98, 0x20b)
        struct.pack_into('<I', data, 0x98 + 120, 0x1000)
        struct.pack_into('<IIII', data, 0x98 + 240 + 8, 512, 0x1000, 512, 512)
        struct.pack_into('<IIIII', data, 512, 0, 0, 0, 0x1080, 1)
        data[640:657] = b'VCRUNTIME140.dll\0'
        self.assertEqual(release.windows_imports(bytes(data)), ['VCRUNTIME140.dll'])
        struct.pack_into('<H', data, 0x84, 0x14c)
        with self.assertRaisesRegex(ValueError, 'not x86-64'):
            release.windows_imports(bytes(data))

    def test_static_crt_audit_refuses_empty_and_external_runtime_imports(self):
        release.validate_windows_imports(['KERNEL32.dll', 'bcrypt.dll'])
        for names in ([], ['VCRUNTIME140.dll'], ['msvcp140.dll'], ['ucrtbase.dll'], ['API-MS-WIN-CRT-stdio-l1-1-0.dll']):
            with self.subTest(names=names), self.assertRaises(ValueError):
                release.validate_windows_imports(names)

    def test_static_linux_audit_refuses_interpreter_needed_and_dynamic_file(self):
        good = ('LOAD', 'There is no dynamic section', 'ELF 64-bit x86-64 static-pie linked', 'not a dynamic executable')
        release.validate_static_linux(*good)
        for changed in (('INTERP', *good[1:]), (good[0], '(NEEDED) libc.so.6', *good[2:]), (*good[:2], 'x86-64 dynamically linked', good[3]), (*good[:3], 'libc.so.6 => /lib/libc.so.6')):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                release.validate_static_linux(*changed)

    def test_v051_immutable_legacy_profile_and_future_five_archive_profile(self):
        self.assertEqual(len(release.legacy_or_current_targets('0.5.1', [])), 4)
        self.assertEqual(len(release.legacy_or_current_targets('0.5.1', [release.asset_name('0.5.1', 'x86_64-unknown-linux-gnu')])), 4)
        self.assertEqual(len(release.legacy_or_current_targets('0.5.2', [])), 5)
        self.assertIn('x86_64-unknown-linux-gnu', release.TARGETS)

    def test_gnu_packaging_fails_closed_on_missing_or_failed_required_runtime_gate(self):
        images = [
            'almalinux:8@sha256:2d0f2eb2a83cc09dbc2eb92fade4dbe8f709628149e81aa00aec70630406603b',
            'centos:7@sha256:be65f488b7764ad3638f236b7b515b3678369a5124c47b8d32916d6487418ea4',
        ]
        report = dict(method='cargo-libc', target_glibc='2.17', cargo_libc_version='cargo-libc 0.7.0',
                      cargo_libc_commit='e68ca7bf3a8ffd973c8576acc48abd0fbcc66d25', source_commit='a'*40, binary_sha256='b'*64,
                      linkage=dict(maximum_required_glibc='2.17', needed=['libc.so.6','libgcc_s.so.1']),
                      runtime_gates=[dict(image=image, result='passed', version_ok=True, plan_ok=True, sync_ok=True) for image in images])
        release.validate_gnu_report(report, 'a'*40, 'b'*64)
        for change in (dict(runtime_gates=report['runtime_gates'][:1]), dict(binary_sha256='c'*64), dict(method='almalinux8'), dict(target_glibc='2.28')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                release.validate_gnu_report(dict(report, **change), 'a'*40, 'b'*64)
        report['runtime_gates'][1]['sync_ok'] = False
        with self.assertRaisesRegex(ValueError, 'gate failed'):
            release.validate_gnu_report(report, 'a'*40, 'b'*64)

    def test_archive_boundary_and_link_refusals(self):
        for suffix in ('.zip', '.tar.gz'):
            with self.subTest(suffix=suffix):
                path, prefix = self.archive(suffix)
                self.assertEqual(set(release.read_archive(path, prefix, 'rust-mirror')), {'rust-mirror', 'LICENSE', 'README.txt'})
                for bad in ('../../secret', prefix + '/link'):
                    path, prefix = self.archive(suffix, bad)
                    with self.assertRaises(ValueError):
                        release.read_archive(path, prefix, 'rust-mirror')

    def test_stage_validates_the_single_manifest(self):
        manifest = self.write_assets()
        with self.assertRaisesRegex(ValueError, 'unexpected or missing'):
            release.staged_assets(self.record, self.root)
        (self.root / release.provenance_name('1.2.3')).write_text(self.bundle_line(self.subjects()))
        (self.root / 'SHA256SUMS').write_text('corrupt')
        with self.assertRaisesRegex(ValueError, 'manifest mismatch'):
            release.staged_assets(self.record, self.root)
        (self.root / 'SHA256SUMS').write_text(manifest)
        names = release.staged_assets(self.record, self.root)
        self.assertEqual(len(names), len(release.TARGETS) + 5)
        self.assertIn('rust-mirror-1.2.3.crate', names)
        self.assertIn('rust-mirror-1.2.3.crate', manifest)

    def write_assets(self):
        names = release.build_summed_assets('1.2.3')
        for name in names:
            (self.root / name).write_bytes(name.encode())
        manifest = ''.join(f'{release.digest(name.encode())}  {name}\n' for name in names)
        (self.root / 'SHA256SUMS').write_text(manifest)
        (self.root / 'SHA256SUMS.asc').write_bytes(b'signature layout fixture')
        return manifest

    def subjects(self):
        return {p.name: release.digest(p.read_bytes()) for p in self.root.iterdir() if not p.name.endswith('.intoto.jsonl') and p.name != 'SHA256SUMS.asc'}

    @staticmethod
    def bundle(subjects, predicate=release.SLSA_PROVENANCE, sig='c2ln'):
        statement = {'_type': 'https://in-toto.io/Statement/v1', 'predicateType': predicate, 'predicate': {},
                     'subject': [{'name': n, 'digest': {'sha256': d}} for n, d in sorted(subjects.items())]}
        payload = __import__('base64').b64encode(json.dumps(statement).encode()).decode()
        return {'mediaType': 'application/vnd.dev.sigstore.bundle.v0.3+json', 'verificationMaterial': {'certificate': {}},
                'dsseEnvelope': {'payload': payload, 'payloadType': 'application/vnd.in-toto+json', 'signatures': [{'sig': sig}]}}

    def bundle_line(self, subjects, **kwargs):
        return json.dumps(self.bundle(subjects, **kwargs)) + '\n'

    def test_provenance_asset_starts_after_immutable_v054(self):
        self.assertFalse(release.has_provenance_asset('0.5.4'))
        self.assertTrue(release.has_provenance_asset('0.5.5'))
        self.assertTrue(release.has_provenance_asset('0.6.0'))
        self.assertEqual(release.provenance_name('0.6.0'), 'rust-mirror-0.6.0.intoto.jsonl')
        self.assertTrue(release.has_provenance_asset('1.0.0-rc.1'))

    def test_stage_adds_signed_provenance_outside_sha256sums(self):
        manifest = self.write_assets()
        subjects = self.subjects()
        bundle = self.root.parent / 'attestation.jsonl'
        bundle.write_text(json.dumps(self.bundle(subjects), indent=2))  # pretty single JSON is normalised
        names = release.staged_assets(self.record, self.root, bundle)
        name = release.provenance_name('1.2.3')
        self.assertIn(name, names)
        self.assertNotIn(name, (self.root / 'SHA256SUMS').read_text())
        self.assertEqual((self.root / 'SHA256SUMS').read_text(), manifest)
        self.assertIn('SHA256SUMS', subjects)
        lines = (self.root / name).read_text().splitlines()
        self.assertEqual(len(lines), 1)
        self.assertEqual(json.loads(lines[0]), self.bundle(subjects))
        self.assertNotIn(b'\r', (self.root / name).read_bytes())
        with self.assertRaisesRegex(ValueError, 'already staged'):
            release.staged_assets(self.record, self.root, bundle)

    def test_staged_provenance_is_lf_only_bytes_from_crlf_input(self):
        self.write_assets()
        subjects = self.subjects()
        bundle = self.root.parent / 'attestation.jsonl'
        lines = [self.bundle_line(subjects).rstrip('\n')] * 2
        bundle.write_bytes(('\r\n'.join(lines) + '\r\n').encode())
        release.staged_assets(self.record, self.root, bundle)
        raw = (self.root / release.provenance_name('1.2.3')).read_bytes()
        self.assertEqual(raw, (json.dumps(self.bundle(subjects), separators=(',', ':')) + '\n').encode() * 2)

    def test_store_match_refuses_when_nothing_was_downloaded(self):
        self.write_assets()
        bundle = self.root.parent / 'attached.jsonl'; bundle.write_bytes(self.bundle_line(self.subjects()).encode())
        with patch.object(release.subprocess, 'run'), self.assertRaisesRegex(ValueError, 'no GitHub-stored provenance'):
            release.verify_attached_provenance(sorted(self.root.iterdir()), bundle, ['--repo', release.REPO], Path(tempfile.mkdtemp(dir=self.temp.name)))

    def test_stage_refuses_incomplete_foreign_or_unsigned_provenance(self):
        self.write_assets()
        subjects = self.subjects()
        missing_sums = {k: v for k, v in subjects.items() if k != 'SHA256SUMS'}
        wrong_digest = dict(subjects, SHA256SUMS='0' * 64)
        unsigned = self.bundle(subjects); unsigned['dsseEnvelope']['signatures'] = []
        cases = {'subjects differ': self.bundle_line(missing_sums), 'subjects differ ': self.bundle_line(wrong_digest),
                 'not SLSA': self.bundle_line(subjects, predicate='https://cyclonedx.org/bom'),
                 'signed in-toto': json.dumps(unsigned) + '\n', 'empty': ''}
        bundle = self.root.parent / 'attestation.jsonl'
        for message, text in cases.items():
            with self.subTest(message=message):
                bundle.write_text(text)
                with self.assertRaisesRegex(ValueError, message.strip()):
                    release.staged_assets(self.record, self.root, bundle)
                (self.root / release.provenance_name('1.2.3')).unlink(missing_ok=True)

    def test_attached_provenance_must_match_github_storage_for_every_asset(self):
        self.write_assets()
        subjects = self.subjects()
        bundle = self.root.parent / 'attached.jsonl'; bundle.write_text(self.bundle_line(subjects))
        assets = [p for p in sorted(self.root.iterdir()) if p.name != 'SHA256SUMS.asc']
        for stored_sig, ok in (('c2ln', True), ('b3RoZXI=', False)):
            calls = []
            def run(args, check, cwd=None, stdout=None):
                calls.append(args[:3])
                if args[2] == 'download':
                    # gh's Windows-safe name; 'sha256:x' would be an NTFS alternate data stream invisible to glob.
                    (Path(cwd) / 'sha256-x.jsonl').write_bytes(self.bundle_line(subjects, sig=stored_sig).encode())
            scratch = Path(tempfile.mkdtemp(dir=self.temp.name))
            with self.subTest(ok=ok), patch.object(release.subprocess, 'run', side_effect=run):
                if ok:
                    release.verify_attached_provenance(assets, bundle, ['--repo', release.REPO], scratch)
                    self.assertEqual(calls.count(['gh', 'attestation', 'verify']), len(assets))
                else:
                    with self.assertRaisesRegex(ValueError, 'GitHub-stored'):
                        release.verify_attached_provenance(assets, bundle, ['--repo', release.REPO], scratch)

    def test_collector_binds_all_targets_to_tag_license_and_original_bytes(self):
        artifacts = self.root / 'artifacts'; artifacts.mkdir()
        for target in release.TARGETS:
            folder = artifacts / ('binary-' + target); folder.mkdir()
            name = release.asset_name('1.2.3', target)
            exe = 'rust-mirror.exe' if 'windows' in target else 'rust-mirror'
            prefix = name.removesuffix('.zip').removesuffix('.tar.gz')
            content = {exe: b'binary', 'LICENSE': b'license', 'README.txt': f"Source commit: {self.record['commit']}\n".encode()}
            path = folder / name
            if name.endswith('.zip'):
                with zipfile.ZipFile(path, 'w') as archive:
                    for member, data in content.items(): archive.writestr(prefix + '/' + member, data)
            else:
                with tarfile.open(path, 'w:gz') as archive:
                    for member, data in content.items():
                        info = tarfile.TarInfo(prefix + '/' + member);info.size = len(data);info.mode = 0o755 if member == exe else 0o644
                        archive.addfile(info, io.BytesIO(data))
            report = dict(target=target, commit=self.record['commit'], tag='v1.2.3', archive=name, archive_smoke=True, sha256=release.digest(path.read_bytes()), binary_sha256=release.digest(b'binary'))
            (folder / 'build-report.json').write_text(json.dumps(report))
        output = self.root / 'output'
        sbom = self.root / 'source.cdx.json'
        self.write_sbom(sbom)
        crate = self.crate_fixture(self.root / 'crates')
        with self.assertRaisesRegex(ValueError, 'missing, misnamed'):
            release.collect(self.root, self.record, artifacts, self.root / 'no-crate', sbom)
        for bad, message in ((self.crate_fixture(self.root / 'dirty', dirty=True), 'crate VCS'), (self.crate_fixture(self.root / 'other', commit='b' * 40), 'crate VCS')):
            with self.assertRaisesRegex(ValueError, message):
                release.collect(self.root, self.record, artifacts, self.root / ('bad-' + bad.parent.name), sbom, crate=bad)
        sums = release.collect(self.root, self.record, artifacts, output, sbom, crate=crate)
        self.assertEqual(len(sums.splitlines()), len(release.TARGETS) + 2)
        self.assertEqual([line.split('  ')[1] for line in sums.splitlines()], release.build_summed_assets('1.2.3'))
        self.assertEqual(sums.splitlines(), sorted(sums.splitlines(), key=lambda line: line.split('  ')[1]))
        self.write_sbom(sbom, commit='b'*40)
        with self.assertRaisesRegex(ValueError, 'SBOM source/lock'):
            release.collect(self.root, self.record, artifacts, self.root / 'wrong-source', sbom, crate=crate)
        self.write_sbom(sbom)
        document=json.loads(sbom.read_text());document['metadata']['properties'][1]['value']='0'*64;sbom.write_text(json.dumps(document))
        with self.assertRaisesRegex(ValueError, 'SBOM source/lock'):
            release.collect(self.root, self.record, artifacts, self.root / 'wrong-lock', sbom, crate=crate)
        self.write_sbom(sbom)
        (artifacts / ('binary-' + release.TARGETS[0]) / 'build-report.json').write_text('{}')
        with self.assertRaises((KeyError, ValueError)):
            release.collect(self.root, self.record, artifacts, self.root / 'bad-output', sbom, crate=crate)


    def crate_fixture(self, folder, commit=None, dirty=False, extra=None):
        folder.mkdir(parents=True, exist_ok=True)
        path = folder / release.crate_name('1.2.3')
        vcs = {'git': {'sha1': commit or self.record['commit']}, 'path_in_vcs': ''}
        if dirty: vcs['git']['dirty'] = True
        files = {'.cargo_vcs_info.json': json.dumps(vcs).encode(), 'Cargo.toml': b'normalized', 'Cargo.toml.orig': b'[package]\n', 'src/lib.rs': b'// lib\n'}
        files.update(extra or {})
        with tarfile.open(path, 'w:gz') as archive:
            for name, data in files.items():
                info = tarfile.TarInfo('rust-mirror-1.2.3/' + name); info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        return path

    def test_published_profiles_keep_v051_to_v060_and_add_crate_after(self):
        archives = lambda v, targets=release.TARGETS: [release.asset_name(v, t) for t in targets]
        no_gnu = [t for t in release.TARGETS if t != 'x86_64-unknown-linux-gnu']
        expected = {
            '0.5.1': archives('0.5.1', no_gnu),
            '0.5.2': archives('0.5.2'),
            '0.5.4': archives('0.5.4') + ['rust-mirror-0.5.4.cdx.json'],
            '0.6.0': archives('0.6.0') + ['rust-mirror-0.6.0.cdx.json'],
            '0.6.1': archives('0.6.1') + ['rust-mirror-0.6.1.cdx.json', 'rust-mirror-0.6.1.crate'],
            '0.7.0': archives('0.7.0') + ['rust-mirror-0.7.0.cdx.json', 'rust-mirror-0.7.0.crate'],
        }
        for version, summed in expected.items():
            with self.subTest(version=version):
                profile = release.published_profile(version, [])
                self.assertEqual(profile['summed'], sorted(summed))
                self.assertEqual(profile['signed'], sorted(summed) + ['SHA256SUMS'])
                bundle = {'0.6.0', '0.6.1', '0.7.0'} >= {version} and f'rust-mirror-{version}.intoto.jsonl'
                self.assertEqual(profile['bundle'], bundle or None)
                self.assertEqual(profile['assets'], set(summed) | {'SHA256SUMS'} | ({bundle} if bundle else set()) | ({'SHA256SUMS.asc'} if version == '0.7.0' else set()))
        self.assertEqual(release.published_profile('0.6.1', [])['summed'], release.build_summed_assets('0.6.1'))
        self.assertEqual(len(release.published_profile('0.6.1', [])['assets']), 9)
        self.assertFalse(release.has_single_profile('0.6.0'))
        self.assertTrue(release.has_single_profile('0.6.1'))

    def test_openpgp_is_required_only_after_current_latest_v061(self):
        for v in ('0.5.4','0.6.0','0.6.1','0.6.1-rc.1'):
            self.assertFalse(release.has_openpgp_signature(v))
            self.assertNotIn('SHA256SUMS.asc',release.published_profile(v,[])['assets'])
        for v in ('0.6.2-rc.1','0.6.2','0.7.0','1.0.0'):
            profile=release.published_profile(v,[])
            self.assertIn('SHA256SUMS.asc',profile['assets'])
            self.assertNotIn('SHA256SUMS.asc',profile['summed'])
            self.assertNotIn('SHA256SUMS.asc',profile['signed'])
            self.assertEqual(len(profile['assets']),10)
        self.write_assets()
        (self.root/release.provenance_name('1.2.3')).write_text(self.bundle_line(self.subjects()))
        (self.root/'SHA256SUMS.asc').unlink()
        with self.assertRaisesRegex(ValueError,'missing'):
            release.staged_assets(self.record,self.root)
        # Tag builds and candidates stage provenance before environment approval.
        # The publishing command still requires the full versioned profile.
        self.assertEqual(len(release.staged_assets(self.record,self.root,signature_required=False)),9)

    def test_publication_signature_failure_precedes_attestations_and_writes(self):
        self.write_assets()
        (self.root/release.provenance_name('1.2.3')).write_text(self.bundle_line(self.subjects()))
        argv=['helper','release','--root',str(self.root),'--tag','v1.2.3',
              '--commit',self.record['commit'],'--assets',str(self.root)]
        with patch.object(release,'record',return_value=self.record), patch.object(release,'remote_commit',return_value=self.record['commit']), \
             patch.object(release.openpgp,'verify',side_effect=ValueError('bad signature')), \
             patch.object(release,'verify_attached_provenance') as attest, patch.object(release,'publish_release') as publish, \
             patch.object(sys,'argv',argv):
            with self.assertRaisesRegex(ValueError,'bad signature'):
                release.main()
            attest.assert_not_called();publish.assert_not_called()
        with patch.object(sys,'argv',argv+['--candidate']), patch('sys.stderr',new=io.StringIO()), self.assertRaises(SystemExit) as error:
            release.main()
        self.assertEqual(error.exception.code,2)

    def test_recorded_live_releases_match_their_published_profiles(self):
        evals = Path(__file__).resolve().parents[1] / 'docs/evals'
        v054 = json.loads((evals / 'publication-0.5.4.json').read_text())
        v060 = json.loads((evals / 'publication-0.6.0.json').read_text())
        live = {tag: set(v054['previous_releases_unchanged'][tag]['assets']) for tag in ('v0.5.1', 'v0.5.2')}
        live['v0.5.4'] = set(v054['release']['assets'])
        live['v0.6.0'] = set(v060['assets'])
        v061 = json.loads((evals / 'publication-0.6.1.json').read_text())
        live['v0.6.1'] = {a['name'] for a in v061['independent_audit']['assets']}
        for tag, names in live.items():
            with self.subTest(tag=tag):
                self.assertEqual(release.published_profile(tag[1:], names)['assets'], names)

    def test_crate_audit_refuses_changed_ci_dirty_or_foreign_content(self):
        def git(*args):
            return subprocess.check_output(['git', *args], cwd=self.root, text=True).strip()
        git('init', '-q'); git('config', 'user.name', 'Fixture'); git('config', 'user.email', 'fixture@example.invalid')
        (self.root / 'src').mkdir(); (self.root / 'src/lib.rs').write_bytes(b'// lib\n'); (self.root / 'Cargo.toml').write_bytes(b'[package]\n')
        git('add', '.'); git('commit', '-qm', 'source'); commit = git('rev-parse', 'HEAD')
        good = self.crate_fixture(self.root / 'good', commit=commit)
        self.assertEqual(release.audit_crate(self.root, commit, good), 4)
        cases = {'differs from the frozen commit': dict(extra={'src/lib.rs': b'// changed\n'}),
                 'fuzz or CI content': dict(extra={'fuzz/Cargo.toml': b'x'}),
                 'clean frozen commit': dict(dirty=True)}
        for message, options in cases.items():
            with self.subTest(message=message):
                bad = self.crate_fixture(self.root / message.replace(' ', '-'), commit=commit, **options)
                with self.assertRaisesRegex(ValueError, message):
                    release.audit_crate(self.root, commit, bad)


    class FakeGitHub:
        """In-memory releases API: records every write so tests can prove none happened."""
        def __init__(self, test, releases=()):
            self.test, self.releases, self.writes, self.next_id = test, [dict(r) for r in releases], [], 100

        def __call__(self, url, method='GET', data=None, content_type=None):
            base = 'https://api.github.com/repos/rustcommons/rust-mirror'
            if method == 'GET' and url.startswith(base + '/releases?'):
                return [dict(r, assets=list(r['assets'])) for r in self.releases] if 'page=1' in url else []
            if method == 'GET' and url.startswith(base + '/releases/tags/'):
                tag = url.rsplit('/', 1)[1]
                found = [r for r in self.releases if r['tag_name'] == tag and not r['draft']]
                if not found: raise AssertionError('tags API returns 404 for drafts')
                return found[0]
            if method == 'GET':
                return next(r for r in self.releases if url.endswith('/releases/%d' % r['id']))
            self.writes.append((method, url))
            if method == 'POST' and url == base + '/releases':
                body = json.loads(data)
                self.next_id += 1
                created = dict(id=self.next_id, tag_name=body['tag_name'], name=body['name'], body=body['body'], draft=body['draft'], prerelease=body['prerelease'], html_url='https://github.com/rustcommons/rust-mirror/releases/tag/untagged-1', assets=[])
                self.releases.append(created)
                return created
            if method == 'POST':
                release_id = int(url.split('/releases/')[1].split('/')[0]); name = url.split('?name=')[1]
                asset = dict(name=name, size=len(data), digest='sha256:' + release.digest(data), state='uploaded')
                next(r for r in self.releases if r['id'] == release_id)['assets'].append(asset)
                return asset
            if method == 'PATCH':
                target = next(r for r in self.releases if url.endswith('/releases/%d' % r['id']))
                self.patch = json.loads(data)
                target.update(self.patch); target.pop('make_latest', None)
                target['html_url'] = 'https://github.com/rustcommons/rust-mirror/releases/tag/' + target['tag_name']
                return target
            raise AssertionError((method, url))

    def release_folder(self):
        folder = self.root / 'dist'; folder.mkdir()
        for name in release.build_summed_assets('1.2.3') + ['SHA256SUMS', 'SHA256SUMS.asc', release.provenance_name('1.2.3')]:
            (folder / name).write_bytes(name.encode())
        return folder

    def releasable(self):
        return dict(self.record, notes='# rust-mirror 1.2.3\n\nReviewed notes\n')

    def draft(self, folder, names, **changes):
        assets = [dict(name=n, size=1, digest='sha256:' + release.digest((folder / n).read_bytes()), state='uploaded') for n in names]
        return dict(dict(id=7, tag_name='v1.2.3', name='rust-mirror 1.2.3', body='# rust-mirror 1.2.3\r\n\r\nReviewed notes\r\n', draft=True, prerelease=False, html_url='https://github.com/rustcommons/rust-mirror/releases/tag/untagged-7', assets=assets), **changes)

    def test_new_release_is_a_draft_until_every_asset_is_uploaded(self):
        folder = self.release_folder(); fake = self.FakeGitHub(self)
        published = release.publish_release(self.releasable(), folder, fake)
        methods = [m for m, _ in fake.writes]
        self.assertEqual(methods[0], 'POST'); self.assertEqual(methods[-1], 'PATCH')
        self.assertEqual(methods.count('POST'), 1 + 10)
        self.assertFalse(published['draft']); self.assertEqual(len(published['assets']), 10)

    def test_killed_run_draft_is_resumed_with_only_missing_assets(self):
        folder = self.release_folder(); names = sorted(p.name for p in folder.iterdir())
        fake = self.FakeGitHub(self, [self.draft(folder, names[:4])])
        release.publish_release(self.releasable(), folder, fake)
        uploaded = sorted(url.split('?name=')[1] for m, url in fake.writes if '?name=' in url)
        self.assertEqual(uploaded, names[4:])
        self.assertFalse(any(url.endswith('/releases') for _, url in fake.writes), 'no second release')
        self.assertEqual(fake.writes[-1][0], 'PATCH')

    def test_existing_release_must_be_exactly_ours_before_any_upload(self):
        folder = self.release_folder(); names = sorted(p.name for p in folder.iterdir())
        changed = self.draft(folder, names[:1]); changed['assets'][0]['digest'] = 'sha256:' + '0' * 64
        cases = {'release body differs': self.draft(folder, [], body='Other notes'),
                 'title differs': self.draft(folder, [], name='other'),
                 'draft/prerelease state': self.draft(folder, [], prerelease=True),
                 'unexpected asset': self.draft(folder, []) | {'assets': [dict(name='extra.bin', digest='sha256:00', state='uploaded')]},
                 'never replacing': changed,
                 'refusing to modify a published release': self.draft(folder, names[:3], draft=False, html_url='https://github.com/rustcommons/rust-mirror/releases/tag/v1.2.3')}
        for message, existing in cases.items():
            with self.subTest(message=message):
                fake = self.FakeGitHub(self, [existing])
                with self.assertRaisesRegex(ValueError, message):
                    release.publish_release(self.releasable(), folder, fake)
                self.assertEqual(fake.writes, [], 'refused before any write')
        fake = self.FakeGitHub(self, [self.draft(folder, []), self.draft(folder, [], id=8)])
        with self.assertRaisesRegex(ValueError, 'several releases'):
            release.publish_release(self.releasable(), folder, fake)

    def test_api_refuses_foreign_origins_before_any_request(self):
        with patch.object(release.urllib.request, 'build_opener', side_effect=AssertionError('no request may be made')):
            for url in ('http://api.github.com/repos/x', 'https://api.github.com.evil.test/x', 'https://example.com/https://api.github.com/', 'https://github.com/repos/x'):
                with self.subTest(url=url), self.assertRaisesRegex(ValueError, 'unexpected GitHub API origin'):
                    release.api(url)

    def test_complete_published_release_rerun_writes_nothing(self):
        folder = self.release_folder(); names = sorted(p.name for p in folder.iterdir())
        fake = self.FakeGitHub(self, [self.draft(folder, names, draft=False, html_url='https://github.com/rustcommons/rust-mirror/releases/tag/v1.2.3')])
        release.publish_release(self.releasable(), folder, fake)
        self.assertEqual(fake.writes, [])

    def test_sbom_bindings_checked_for_archives_and_crate(self):
        folder = self.release_folder(); sbom = folder / release.sbom_name('1.2.3')
        sbom.write_text('{"bomFormat": "CycloneDX"}')
        crate = folder / release.crate_name('1.2.3')
        def proof(predicate_document, path):
            return json.dumps([{'verificationResult': {'statement': {'predicateType': 'https://cyclonedx.org/bom', 'predicate': predicate_document,
                                                                     'subject': [{'digest': {'sha256': release.digest(path.read_bytes())}}]}}}])
        with patch.object(release.subprocess, 'check_output', side_effect=lambda args, **kw: proof({'bomFormat': 'CycloneDX'}, Path(args[3]))):
            release.verify_sbom_bindings([crate], sbom, ['--repo', release.REPO])
        with patch.object(release.subprocess, 'check_output', side_effect=lambda args, **kw: proof({'other': 1}, Path(args[3]))):
            with self.assertRaisesRegex(ValueError, 'does not bind this exact SBOM'):
                release.verify_sbom_bindings([crate], sbom, ['--repo', release.REPO])


    def test_archive_readme_prints_the_strict_install_guide_command(self):
        record = dict(self.record, tag='v1.2.3', version='1.2.3')
        text = release.verify_command(record, 'rust-mirror-1.2.3-x86_64-unknown-linux-musl.tar.gz')
        for flag in ('--repo rustcommons/rust-mirror', '--signer-workflow rustcommons/rust-mirror/.github/workflows/build-binaries.yml',
                     '--source-digest ' + 'a' * 40, '--signer-digest ' + 'a' * 40, '--deny-self-hosted-runners',
                     '--predicate-type https://slsa.dev/provenance/v1'):
            self.assertIn(flag, text)
        install = (Path(__file__).resolve().parents[1] / 'docs/INSTALL.md').read_text()
        for flag in ('--signer-workflow rustcommons/rust-mirror/.github/workflows/build-binaries.yml',
                     '--source-digest "$commit" --signer-digest "$commit" --deny-self-hosted-runners'):
            self.assertIn(flag, install)
        source = (Path(__file__).resolve().parents[1] / 'scripts/release_binaries.py').read_text()
        self.assertIn("{verify_command(r, asset_name(r['version'], args.target))}", source)
        self.assertNotIn("--repo {REPO}\nInstructions", source)

    def test_latest_only_for_the_highest_final_release(self):
        published = lambda tag, **kw: dict(dict(tag_name=tag, draft=False, prerelease=False), **kw)
        self.assertTrue(release.is_highest_release('v1.2.3', [published('v1.2.2'), published('v0.9.0')]))
        self.assertTrue(release.is_highest_release('v1.2.3', [published('v1.2.3', draft=True), published('v2.0.0', draft=True)]))
        self.assertFalse(release.is_highest_release('v1.2.3', [published('v1.3.0')]))
        self.assertFalse(release.is_highest_release('v1.2.3', [published('v1.10.0')]), 'numeric, not lexical')
        self.assertTrue(release.is_highest_release('v1.10.0', [published('v1.9.0')]))
        self.assertFalse(release.is_highest_release('v1.3.0-rc.1', []))
        self.assertTrue(release.is_highest_release('v1.2.3', [published('v9.0.0-rc.1'), published('v9.0.0', prerelease=True)]))

    def test_publish_marks_latest_only_when_highest(self):
        for others, latest in (([], 'true'), ([dict(id=50, tag_name='v2.0.0', name='x', body='', draft=False, prerelease=False, html_url='', assets=[])], 'false')):
            with self.subTest(latest=latest):
                folder = self.root / ('dist-' + latest); folder.mkdir()
                for name in release.build_summed_assets('1.2.3') + ['SHA256SUMS', 'SHA256SUMS.asc', release.provenance_name('1.2.3')]:
                    (folder / name).write_bytes(name.encode())
                fake = self.FakeGitHub(self, others)
                release.publish_release(self.releasable(), folder, fake)
                self.assertEqual(fake.patch, {'draft': False, 'make_latest': latest})


if __name__ == '__main__': unittest.main()
