"""Security-sensitive release refusal and measurement controls; no live writes."""
import hashlib
import io
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import release_binaries as release
import release_sbom
import verify_registry_release as registry
import compare_reproducible as repro


class SecurityReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='rust-mirror-security-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_future_attestation_requires_exact_source_and_signer_commits(self):
        commit = 'a'*40
        flags = release.attestation_policy({'version':'1.2.3', 'commit':commit})
        for option in ('--source-digest', '--signer-digest'):
            self.assertEqual(flags[flags.index(option)+1], commit)
        self.assertIn('--deny-self-hosted-runners', flags)
        self.assertTrue(flags[flags.index('--signer-workflow')+1].endswith('/build-binaries.yml'))
        with self.assertRaises(ValueError):
            release.attestation_policy({'version':'1.2.3', 'commit':'main'})
        for version in ('0.5.1', '0.5.2'):
            legacy = release.attestation_policy({'version':version, 'commit':commit})
            self.assertNotIn('--source-digest', legacy)
            self.assertNotIn('--signer-digest', legacy)
            self.assertTrue(legacy[-1].endswith('/release-binaries.yml'))

    def test_sbom_refuses_wrong_format_root_and_incomplete_graph(self):
        good = {'bomFormat': 'CycloneDX', 'specVersion': '1.5', 'metadata': {'component': {'name': 'rust-mirror', 'version': '1.2.3'}}, 'components': [{'name': 'dep', 'version': '1.0.0'}]}
        release_sbom.validate(good, 'rust-mirror', '1.2.3')
        for changed in (dict(bomFormat='SPDX'), dict(specVersion='1.3'), dict(metadata={}), dict(components=[]), dict(components=[{'name': 'dep'}]), dict(components=[{}]*10001)):
            with self.subTest(changed=list(changed)), self.assertRaises(ValueError):
                release_sbom.validate(dict(good, **changed), 'rust-mirror', '1.2.3')

    def test_sbom_canonicalization_preserves_references_and_removes_only_scratch_ids(self):
        a = {'metadata': {'component': {'bom-ref': 'path+file:///tmp/a#root', 'purl': 'pkg:cargo/root@1?download_url=file://.'}}, 'dependencies': [{'ref': 'path+file:///tmp/a#root'}], 'other': '/unrelated/file', 'description': '?download_url=file://.'}
        b = json.loads(json.dumps(a).replace('/tmp/a', '/tmp/b'))
        first = release_sbom.canonicalize(a, PurePosixPath('/tmp/a'), 'a'*40, 'b'*64)
        second = release_sbom.canonicalize(b, PurePosixPath('/tmp/b'), 'a'*40, 'b'*64)
        self.assertEqual(first, second)
        self.assertEqual(first['metadata']['component']['bom-ref'], first['dependencies'][0]['ref'])
        self.assertEqual(first['other'], '/unrelated/file')
        self.assertEqual(first['description'], '?download_url=file://.')
        for path, uri in [(PureWindowsPath('C:/tmp/a'), 'path+file:///C:/tmp/a#root'), (PureWindowsPath('C:/space dir'), 'path+file:///C:/space%20dir#root')]:
            result = release_sbom.canonicalize({'metadata': {'component': {'bom-ref': uri}}, 'dependencies': [{'ref': uri}]}, path, 'a'*40, 'b'*64)
            self.assertEqual(result['metadata']['component']['bom-ref'], first['metadata']['component']['bom-ref'])
            self.assertEqual(result['metadata']['component']['bom-ref'], result['dependencies'][0]['ref'])
        self.assertEqual(first['metadata']['properties'][0]['value'], 'a'*40)

    def test_verified_sbom_predicate_must_match_document_and_archive_digest(self):
        document = {'bomFormat': 'CycloneDX', 'version': 1}
        proof = [{'verificationResult': {'statement': {'predicateType': 'https://cyclonedx.org/bom', 'predicate': document, 'subject': [{'digest': {'sha256': 'a'*64}}]}}}]
        release_sbom.validate_binding(proof, document, 'a'*64)
        for doc, digest in [({'version': 2}, 'a'*64), (document, 'b'*64)]:
            with self.assertRaises(ValueError): release_sbom.validate_binding(proof, doc, digest)
        with self.assertRaises(ValueError): release_sbom.validate_binding([], document, 'a'*64)

    def fixture(self):
        def git(*args):
            return subprocess.check_output(['git', *args], cwd=self.root, text=True).strip()
        git('init', '-q');git('config','user.name','Release fixture');git('config','user.email','fixture@example.invalid')
        (self.root/'Cargo.toml').write_text('[package]\nname="rust-mirror"\nversion="1.2.3"\nrust-version="1.90"\n')
        (self.root/'Cargo.lock').write_text('fixture lock')
        for p in ['LICENSE','README.md','CHANGELOG.md']:(self.root/p).write_text(p)
        (self.root/'src').mkdir();(self.root/'src/lib.rs').write_text('// fixture')
        (self.root/'docs/releases').mkdir(parents=True);(self.root/'docs/releases/v1.2.3.md').write_text('Notes\n')
        git('add','.');git('commit','-qm','Fixture');git('tag','-a','v1.2.3','-m','Fixture')
        r=release.record(self.root,'v1.2.3')
        files={p:subprocess.check_output(['git','show',r['commit']+':'+p],cwd=self.root) for p in ['Cargo.toml','Cargo.lock','LICENSE','README.md','CHANGELOG.md','src/lib.rs']}
        files['Cargo.toml.orig']=files['Cargo.toml'];files['.cargo_vcs_info.json']=json.dumps({'git':{'sha1':r['commit']}}).encode()
        return r,files

    def archive(self, files, duplicate=False):
        output=io.BytesIO()
        with tarfile.open(fileobj=output,mode='w:gz') as t:
            for name,data in list(files.items())+([('README.md',files['README.md'])] if duplicate else []):
                m=tarfile.TarInfo('rust-mirror-1.2.3/'+name);m.size=len(data);t.addfile(m,io.BytesIO(data))
        return output.getvalue()

    def test_registry_payload_refuses_source_tampering_dirty_vcs_and_extra_paths(self):
        r,files=self.fixture();registry.verify_payload(self.archive(files),self.root,r)
        for changed in ({'README.md':b'tampered'}, {'.cargo_vcs_info.json':json.dumps({'git':{'sha1':r['commit'],'dirty':True}}).encode()}, {'.cargo_vcs_info.json':json.dumps({'git':{'sha1':'0'*40}}).encode()}, {'../escape':b'x'}, {'unexpected':b'x'}):
            with self.subTest(changed=list(changed)), self.assertRaises(ValueError):
                registry.verify_payload(self.archive(dict(files,**changed)),self.root,r)
        with self.assertRaisesRegex(ValueError,'duplicate'):
            registry.verify_payload(self.archive(files,True),self.root,r)

    def test_registry_checksum_gate_and_existing_manual_release_are_read_only(self):
        r,files=self.fixture();raw=self.archive(files);sha=hashlib.sha256(raw).hexdigest()
        responses={
            'https://crates.io/api/v1/crates/rust-mirror/1.2.3':json.dumps({'version':{'checksum':sha,'yanked':False}}).encode(),
            'https://index.crates.io/ru/st/rust-mirror':json.dumps({'vers':'1.2.3','cksum':sha,'yanked':False}).encode(),
            'https://static.crates.io/crates/rust-mirror/rust-mirror-1.2.3.crate':raw,
        }
        def get(url): return responses[url]
        with patch.object(registry,'get',side_effect=get):
            result=registry.check(self.root,'v1.2.3');self.assertTrue(result['published']);self.assertEqual(result['archive_sha256'],sha)
        with patch.object(registry,'get',return_value=json.dumps({'version':{'checksum':'0'*64,'yanked':True}}).encode()),self.assertRaises(ValueError):
            registry.check(self.root,'v1.2.3')
        missing=urllib.error.HTTPError('https://crates.io',404,'missing',None,None)
        with patch.object(registry,'get',side_effect=missing) as request:
            self.assertFalse(registry.check(self.root,'v1.2.3',allow_missing=True)['published']);self.assertEqual(request.call_count,1)
        with patch.object(registry,'get',side_effect=missing),self.assertRaises(ValueError):registry.check(self.root,'v1.2.3')

    def write_archives(self, folder, binary):
        folder.mkdir()
        for target in release.TARGETS:
            name=release.asset_name('1.2.3',target);exe='rust-mirror.exe' if 'windows' in target else 'rust-mirror'
            prefix=name.removesuffix('.zip').removesuffix('.tar.gz');files={exe:binary,'LICENSE':b'license','README.txt':b'source'}
            if name.endswith('.zip'):
                with zipfile.ZipFile(folder/name,'w') as z:
                    for n,b in files.items():z.writestr(prefix+'/'+n,b)
            else:
                with tarfile.open(folder/name,'w:gz') as t:
                    for n,b in files.items():
                        m=tarfile.TarInfo(prefix+'/'+n);m.size=len(b);m.mode=0o755 if n==exe else 0o644;t.addfile(m,io.BytesIO(b))
        (folder/release.sbom_name('1.2.3')).write_text('{}')

    def test_reproducibility_does_not_silently_accept_different_executables(self):
        self.write_archives(self.root/'a',b'first executable');shutil.copytree(self.root/'a',self.root/'b')
        self.assertTrue(repro.compare(self.root/'a',self.root/'b','1.2.3')['all_bit_identical'])
        self.write_archives(self.root/'c',b'different executable')
        result=repro.compare(self.root/'a',self.root/'c','1.2.3');self.assertFalse(result['all_bit_identical'])
        self.assertTrue(all(not r['executable_identical'] for r in result['rows']))
        self.assertTrue(all('root cause requires review' in r['reason'] for r in result['rows']))

    def test_pe_timestamp_analysis_never_changes_raw_identity(self):
        raw=bytearray(128);raw[:2]=b'MZ';struct.pack_into('<I',raw,0x3c,64);raw[64:68]=b'PE\0\0';struct.pack_into('<I',raw,72,100)
        other=bytearray(raw);struct.pack_into('<I',other,72,200)
        self.assertNotEqual(repro.digest(raw),repro.digest(other))
        self.assertEqual(repro.metadata(raw)['metadata_normalized_sha256'],repro.metadata(other)['metadata_normalized_sha256'])


if __name__=='__main__':unittest.main()
