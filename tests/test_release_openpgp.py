"""Release key identity and real GnuPG controls; all private material is throwaway."""
import contextlib
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import release_openpgp as pgp


class ReleaseOpenPGPTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.stack = contextlib.ExitStack()
        cls.addClassCleanup(cls.stack.close)
        home = cls.stack.enter_context(pgp.temporary_home())
        cls.passphrase = 'throwaway release test passphrase'
        common = ('--pinentry-mode', 'loopback', '--passphrase-fd', '0')
        def generate(*args):
            result = pgp.gpg(home, *common, *args, data=(cls.passphrase+'\n').encode())
            if result.returncode:
                raise AssertionError('throwaway GnuPG key generation failed: '+result.stderr.decode())
            return result.stdout
        generate('--quick-generate-key', 'Release fixture <release@example.invalid>', 'rsa2048', 'cert', '1d')
        cls.primary = pgp.primary_fingerprints(home)[0]
        generate('--quick-add-key', cls.primary, 'rsa2048', 'sign', '1d')
        rows = pgp.gpg(home, '--with-colons', '--fingerprint', '--list-keys').stdout.decode().splitlines()
        cls.subkey = [line.split(':')[9] for line in rows if line.startswith('fpr:')][1]
        cls.public = generate('--armor', '--export', cls.primary)
        cls.secret = generate('--armor', '--export-secret-subkeys', cls.primary)

    def setUp(self):
        self.stack = contextlib.ExitStack()
        self.addCleanup(self.stack.close)
        self.folder = Path(self.stack.enter_context(tempfile.TemporaryDirectory(prefix='rm-pgp-files-')))
        self.sign_home = self.stack.enter_context(pgp.temporary_home())
        self.verify_home = self.stack.enter_context(pgp.temporary_home())
        pgp.import_key(self.sign_home, self.secret)
        pgp.import_key(self.verify_home, self.public)
        self.manifest = self.folder / 'SHA256SUMS'
        (self.folder / 'payload.crate').write_bytes(b'fixture payload')
        import hashlib
        self.manifest.write_bytes((hashlib.sha256(b'fixture payload').hexdigest()+'  payload.crate\n').encode())
        self.signature = self.folder / pgp.SIGNATURE

    def sign(self):
        pgp._detach_sign(self.sign_home, self.manifest, self.signature, self.subkey, self.passphrase)

    def test_committed_key_primary_fingerprint_is_pinned(self):
        self.assertEqual(pgp.PRIMARY_FINGERPRINT, 'B281C3010A76A11737B4D066C7396F6EA7647CC6')
        with pgp.temporary_home() as home:
            pgp.pinned_key(home)
            self.assertEqual(pgp.primary_fingerprints(home), [pgp.PRIMARY_FINGERPRINT])
            self.assertEqual(pgp.SIGNING_SUBKEY,'248DFD639B75D1EFF2A87289D7C7DE7BBCCF35EC')
            self.assertIn(pgp.SIGNING_SUBKEY,pgp.gpg(home,'--with-colons','--fingerprint','--list-keys').stdout.decode())

    def test_ephemeral_subkey_signs_and_separate_public_home_verifies(self):
        self.sign()
        proof = pgp._verify_loaded(self.verify_home, self.manifest, self.signature, self.primary)
        self.assertEqual(proof['primary_fingerprint'], self.primary)
        self.assertEqual(proof['signing_fingerprint'], self.subkey)
        self.assertNotEqual(self.primary, self.subkey)
        self.assertFalse(any(line.startswith(b'sec:') for line in pgp.gpg(self.verify_home, '--with-colons', '--list-secret-keys').stdout.splitlines()))
        pgp.check_all(self.manifest, ['payload.crate'])

    def test_tampered_manifest_bad_missing_or_wrong_key_signature_refuses(self):
        self.sign()
        self.manifest.write_bytes(self.manifest.read_bytes()+b'changed')
        with self.assertRaises(ValueError):
            pgp._verify_loaded(self.verify_home, self.manifest, self.signature, self.primary)
        self.signature.write_bytes(b'not a signature')
        with self.assertRaises(ValueError):
            pgp._verify_loaded(self.verify_home, self.manifest, self.signature, self.primary)
        self.signature.unlink()
        with self.assertRaises(ValueError):
            pgp._verify_loaded(self.verify_home, self.manifest, self.signature, self.primary)

    def test_primary_pin_cannot_be_replaced_by_a_valid_test_signature(self):
        self.sign()
        with self.assertRaisesRegex(ValueError, 'fingerprint mismatch'):
            pgp._verify_loaded(self.verify_home, self.manifest, self.signature, pgp.PRIMARY_FINGERPRINT)
        with self.assertRaises(ValueError):
            pgp.verify(self.manifest, self.signature)
        argv = [sys.executable, str(Path(pgp.__file__).resolve()), 'verify', '--manifest', str(self.manifest), '--signature', str(self.signature)]
        env = dict(os.environ, RELEASE_OPENPGP_TEST_KEY=self.primary, PRIMARY_FINGERPRINT=self.primary)
        self.assertNotEqual(subprocess.run(argv, env=env, capture_output=True).returncode, 0)
        self.assertNotEqual(subprocess.run(argv+['--fingerprint', self.primary], capture_output=True).returncode, 0)

    def test_production_sign_rejects_test_key_before_signing(self):
        with pgp.temporary_home() as signing, pgp.temporary_home() as checking:
            with patch.dict(os.environ, RELEASE_SIGNING_KEY=self.secret.decode(), RELEASE_SIGNING_PASSPHRASE=self.passphrase):
                with patch.object(pgp, '_detach_sign') as sign, self.assertRaisesRegex(ValueError, 'fingerprint mismatch'):
                    pgp.sign(self.manifest, signing, checking)
                sign.assert_not_called()
        self.assertFalse(self.signature.exists())

    def test_signing_uses_stdin_not_argv_and_does_not_inherit_secrets(self):
        with patch.object(pgp.subprocess, 'run', return_value=subprocess.CompletedProcess([],0,b'',b'')) as run:
            with patch.dict(os.environ, RELEASE_SIGNING_KEY='private sentinel', RELEASE_SIGNING_PASSPHRASE='pass sentinel'):
                pgp._detach_sign(self.sign_home,self.manifest,self.signature,self.subkey,'pass sentinel')
        args, kwargs = run.call_args
        self.assertNotIn('pass sentinel', str(args))
        self.assertEqual(kwargs['input'], b'pass sentinel\n')
        self.assertNotIn('RELEASE_SIGNING_KEY',kwargs['env'])
        self.assertNotIn('RELEASE_SIGNING_PASSPHRASE',kwargs['env'])
        self.assertEqual(args[0][args[0].index('--local-user')+1],self.subkey+'!')
        self.assertIn('--passphrase-fd',args[0])

    def test_retry_reuses_only_a_verified_signature_for_identical_manifest(self):
        self.sign()
        prior=self.folder/'prior.asc'
        prior.write_bytes(self.signature.read_bytes())
        original=prior.read_bytes()
        self.assertTrue(pgp._retain_first(self.verify_home,self.manifest,self.signature,prior,self.primary))
        self.assertEqual(self.signature.read_bytes(),original)
        self.assertFalse(pgp._retain_first(self.verify_home,self.manifest,self.signature,None,self.primary))
        prior.write_bytes(b'damaged signature')
        with self.assertRaises(ValueError):
            pgp._retain_first(self.verify_home,self.manifest,self.signature,prior,self.primary)
        self.assertEqual(self.signature.read_bytes(),original)
        prior.write_bytes(original)
        self.manifest.write_bytes(b'changed manifest')
        with self.assertRaises(ValueError):
            pgp._retain_first(self.verify_home,self.manifest,self.signature,prior,self.primary)

    def test_public_verifier_preserves_old_releases_and_requires_new_signature_first(self):
        import release_binaries as release
        import verify_release_binaries as verifier
        with patch.object(pgp,'verify') as verify, patch.object(pgp,'check_all') as check:
            self.assertIsNone(verifier.verify_openpgp(self.folder,release.published_profile('0.6.1',[])))
            verify.assert_not_called();check.assert_not_called()
        profile=release.published_profile('0.6.2',[])
        with patch.object(pgp,'verify',side_effect=ValueError('bad signature')) as verify, patch.object(pgp,'check_all') as check:
            with self.assertRaises(ValueError):
                verifier.verify_openpgp(self.folder,profile)
            check.assert_not_called()
        with patch.object(pgp,'verify',return_value={'primary_fingerprint':pgp.PRIMARY_FINGERPRINT}) as verify, patch.object(pgp,'check_all') as check:
            proof=verifier.verify_openpgp(self.folder,profile)
            self.assertEqual(proof['primary_fingerprint'],pgp.PRIMARY_FINGERPRINT)
            check.assert_called_once_with(self.folder/'SHA256SUMS',profile['summed'])

    def test_workflow_signs_unconditionally_before_auth_and_limits_secret_scope(self):
        text=(Path(pgp.__file__).resolve().parents[1]/'.github/workflows/trusted-publishing.yml').read_text()
        publish=text.split('\n  publish:',1)[1].split('\n  github-release:',1)[0]
        self.assertLess(publish.index('scripts/release_openpgp.py sign'),publish.index('uses: rust-lang/crates-io-auth-action'))
        signing=publish.split('      - name: Sign SHA256SUMS',1)[1].split('      - name:',1)[0]
        self.assertNotIn('        if:',signing)
        self.assertIn('mktemp -d',signing)
        self.assertIn('chmod 700',signing)
        self.assertIn('trap cleanup EXIT',signing)
        self.assertIn('gpgconf --homedir',signing)
        self.assertEqual(text.count('${{ secrets.RELEASE_SIGNING_PASSPHRASE }}'),1)
        self.assertEqual(text.count('${{ secrets.RELEASE_SIGNING_KEY }}'),1)
        for line in publish.splitlines():
            if 'cargo package ' in line or 'cargo publish ' in line:
                self.assertIn('--no-verify',line)

    def test_bad_passphrase_fails_signing(self):
        with self.assertRaisesRegex(ValueError,'signing failed'):
            pgp._detach_sign(self.sign_home,self.manifest,self.signature,self.subkey,'wrong passphrase')

    def test_checksums_verify_every_listed_payload_and_reject_foreign_paths(self):
        pgp.check_all(self.manifest,['payload.crate'])
        (self.folder/'payload.crate').write_bytes(b'damage')
        with self.assertRaises(subprocess.CalledProcessError):
            pgp.check_all(self.manifest,['payload.crate'])
        self.manifest.write_text('0'*64+'  ../foreign\n')
        with self.assertRaises(ValueError):
            pgp.check_all(self.manifest,['payload.crate'])

    def test_validsig_requires_primary_identity_single_signature_and_success(self):
        valid = f'[GNUPG:] VALIDSIG {self.subkey} 2026-11-25 1 0 4 0 1 10 00 {self.primary}\n'.encode()
        self.assertEqual(pgp.valid_signature(valid,0,self.primary)['primary_fingerprint'],self.primary)
        for status, code in [(valid,1),(valid+valid,0),(b'[GNUPG:] GOODSIG anything\n',0),
                             (valid+b'[GNUPG:] BADSIG x\n',0)]:
            with self.subTest(status=status,code=code), self.assertRaises(ValueError):
                pgp.valid_signature(status,code,self.primary)


if __name__ == '__main__':
    unittest.main()
