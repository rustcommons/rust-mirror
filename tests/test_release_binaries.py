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

SPEC = importlib.util.spec_from_file_location('release_binaries', Path(__file__).resolve().parents[1] / 'scripts/release_binaries.py')
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class ReleaseBinariesTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='rust-mirror-release-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.record = dict(tag='v1.2.3', version='1.2.3', commit='a' * 40, notes='Reviewed notes\n', license=b'license')
        self.live = dict(tag_name='v1.2.3', draft=False, html_url='https://github.com/rustcommons/rust-mirror/releases/tag/v1.2.3', body='Reviewed notes\n', assets=[], id=42)

    def test_source_record_uses_tag_not_changed_worktree(self):
        def git(*args):
            subprocess.run(['git', *args], cwd=self.root, check=True, capture_output=True)
        git('init', '-q')
        git('config', 'user.name', 'Release fixture')
        git('config', 'user.email', 'fixture@example.invalid')
        (self.root / 'Cargo.toml').write_text('[package]\nname="rust-mirror"\nversion="1.2.3"\nrust-version="1.90"\n')
        (self.root / 'LICENSE').write_bytes(b'license')
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
        names = sorted(release.asset_name('1.2.3', target) for target in release.TARGETS)
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
        self.assertEqual(methods, ['GET'] + ['POST'] * 5)

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
        sums = release.collect(self.root, self.record, artifacts, output)
        self.assertEqual(len(sums.splitlines()), 4)
        self.assertEqual(sums.splitlines(), sorted(sums.splitlines(), key=lambda line: line.split('  ')[1]))
        (artifacts / ('binary-' + release.TARGETS[0]) / 'build-report.json').write_text('{}')
        with self.assertRaises((KeyError, ValueError)):
            release.collect(self.root, self.record, artifacts, self.root / 'bad-output')


if __name__ == '__main__': unittest.main()
