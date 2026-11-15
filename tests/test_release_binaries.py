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

    def test_candidate_flags_are_never_accepted_by_upload_or_preflight(self):
        for verb in ('upload', 'preflight'):
            for flag in ('--candidate', '--candidate-commit'):
                with self.subTest(verb=verb, flag=flag), patch.object(release, 'api', side_effect=AssertionError('must never reach API')), patch('sys.stderr', new=io.StringIO()):
                    argv = ['helper', verb, '--root', str(self.root), '--tag', 'v1.2.3', '--commit', 'a'*40, flag]
                    if verb == 'upload': argv += ['--output', str(self.root)]
                    with patch.object(sys, 'argv', argv), self.assertRaises(SystemExit) as error:
                        release.main()
                    self.assertEqual(error.exception.code, 2)

    def test_downloaded_gnu_abi_gate_refuses_newer_symbols_and_wrong_linkage(self):
        good = ('INTERP', '(NEEDED) Shared library: [libc.so.6]\n(NEEDED) Shared library: [libgcc_s.so.1]', 'GLIBC_2.2.5 GLIBC_2.17', 'ELF x86-64 dynamically linked')
        self.assertEqual(release.validate_dynamic_linux(*good)['maximum_required_glibc'], '2.17')
        for changed in (('', *good[1:]), (good[0], '', *good[2:]), (*good[:2], 'GLIBC_2.18', good[3]), (*good[:2], '', good[3]), (*good[:3], 'ELF ARM')):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                release.validate_dynamic_linux(*changed)

    def write_sbom(self, path, commit=None):
        document = {'bomFormat': 'CycloneDX', 'specVersion': '1.5',
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

    def test_existing_assets_refuse_live_but_allow_read_only_preflight(self):
        with patch.object(release, 'api', return_value=dict(self.live, assets=[{'name': 'unrelated.txt'}])), patch.object(release, 'remote_commit', return_value=self.record['commit']):
            with self.assertRaisesRegex(ValueError, 'already has assets'):
                release.check_release(self.root, self.record)
            release.check_release(self.root, self.record, allow_existing=True)

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

    def test_upload_refuses_preexisting_before_any_post(self):
        def api(url, method='GET', *args):
            self.assertEqual(method, 'GET', 'must refuse before any write')
            return dict(self.live, assets=[{'name': 'SHA256SUMS'}])
        with patch.object(release, 'record', return_value=self.record), patch.object(release, 'remote_commit', return_value=self.record['commit']), patch.object(release, 'api', side_effect=api), patch.object(sys, 'argv', ['helper', 'upload', '--root', str(self.root), '--tag', 'v1.2.3', '--commit', self.record['commit'], '--output', str(self.root)]):
            with self.assertRaisesRegex(ValueError, 'already has assets'):
                release.main()

    def test_upload_validates_manifest_before_post_and_never_deletes(self):
        names = sorted([release.asset_name('1.2.3', target) for target in release.TARGETS] + [release.sbom_name('1.2.3')])
        for name in names:
            (self.root / name).write_bytes(b'archive')
        manifest = ''.join(f'{release.digest(b"archive")}  {name}\n' for name in names)
        (self.root / 'SHA256SUMS').write_text(manifest)
        methods = []
        def api(url, method='GET', data=None, content_type=None):
            methods.append(method)
            if method == 'GET': return self.live
            self.assertEqual(method, 'POST')
            name = url.split('?name=')[1]
            return dict(name=name, size=len(data), digest='sha256:' + release.digest(data))
        argv = ['helper', 'upload', '--root', str(self.root), '--tag', 'v1.2.3', '--commit', self.record['commit'], '--output', str(self.root)]
        with patch.object(release, 'record', return_value=self.record), patch.object(release, 'remote_commit', return_value=self.record['commit']), patch.object(release, 'api', side_effect=api), patch.object(sys, 'argv', argv):
            (self.root / 'SHA256SUMS').write_text('corrupt')
            with self.assertRaisesRegex(ValueError, 'manifest mismatch'):
                release.main()
            self.assertEqual(methods, ['GET'])
            methods.clear(); (self.root / 'SHA256SUMS').write_text(manifest)
            release.main()
        self.assertEqual(methods, ['GET'] + ['POST'] * (len(release.TARGETS) + 2))

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
        sums = release.collect(self.root, self.record, artifacts, output, sbom)
        self.assertEqual(len(sums.splitlines()), len(release.TARGETS) + 1)
        self.assertEqual(sums.splitlines(), sorted(sums.splitlines(), key=lambda line: line.split('  ')[1]))
        self.write_sbom(sbom, commit='b'*40)
        with self.assertRaisesRegex(ValueError, 'SBOM source/lock'):
            release.collect(self.root, self.record, artifacts, self.root / 'wrong-source', sbom)
        self.write_sbom(sbom)
        document=json.loads(sbom.read_text());document['metadata']['properties'][1]['value']='0'*64;sbom.write_text(json.dumps(document))
        with self.assertRaisesRegex(ValueError, 'SBOM source/lock'):
            release.collect(self.root, self.record, artifacts, self.root / 'wrong-lock', sbom)
        self.write_sbom(sbom)
        (artifacts / ('binary-' + release.TARGETS[0]) / 'build-report.json').write_text('{}')
        with self.assertRaises((KeyError, ValueError)):
            release.collect(self.root, self.record, artifacts, self.root / 'bad-output', sbom)


if __name__ == '__main__': unittest.main()
