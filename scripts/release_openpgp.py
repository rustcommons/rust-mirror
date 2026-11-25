#!/usr/bin/env python3
"""Pinned OpenPGP release checksums. Production commands have no trust override."""
import argparse
import contextlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

PRIMARY_FINGERPRINT = 'B281C3010A76A11737B4D066C7396F6EA7647CC6'
SIGNING_SUBKEY = '248DFD639B75D1EFF2A87289D7C7DE7BBCCF35EC'
PUBLIC_KEY = Path(__file__).resolve().parents[1] / '.github/release-signing-key.asc'
SIGNATURE = 'SHA256SUMS.asc'
MAX_SIGNATURE = 64 * 1024


def require(ok, message):
    if not ok:
        raise ValueError(message)


def tool(name):
    found = shutil.which(name)
    if found:
        return found
    # Git for Windows supplies these tools; no download or install fallback.
    if os.name == 'nt':
        git = shutil.which('git')
        if git:
            candidate = Path(git).resolve().parent.parent / 'usr/bin' / (name + '.exe')
            if candidate.is_file():
                return str(candidate)
    raise ValueError(f'preinstalled {name} is required')


def environment():
    return {k:v for k,v in os.environ.items() if k not in
            ('RELEASE_SIGNING_KEY', 'RELEASE_SIGNING_PASSPHRASE', 'GNUPGHOME', 'GH_TOKEN',
             'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'ACTIONS_ID_TOKEN_REQUEST_TOKEN')} | {'LC_ALL':'C'}


def gpg(home, *args, data=None):
    return subprocess.run([tool('gpg'), '--no-options', '--homedir', str(home), '--batch', '--no-tty',
        '--no-auto-key-retrieve', '--auto-key-locate', 'clear', *args], input=data,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=environment(), timeout=120)


def kill_agent(home):
    subprocess.run([tool('gpgconf'), '--homedir', str(home), '--kill', 'gpg-agent'],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=environment(), timeout=15)


@contextlib.contextmanager
def temporary_home():
    with tempfile.TemporaryDirectory(prefix='rm-gpg-') as directory:
        home = Path(directory)
        home.chmod(0o700)
        try:
            yield home
        finally:
            kill_agent(home)


def empty_home(home):
    require(home.is_dir() and not home.is_symlink() and not any(home.iterdir()), 'GnuPG home must be a fresh empty directory')
    if os.name != 'nt':
        require(home.stat().st_mode & 0o777 == 0o700, 'GnuPG home must have mode 700')


def import_key(home, armor):
    require(0 < len(armor) <= 128 * 1024, 'missing or oversized signing key')
    result = gpg(home, '--import', data=armor)
    # Never include private import input or subprocess diagnostics in an error.
    require(result.returncode == 0, 'GnuPG key import failed')


def primary_fingerprints(home):
    result = gpg(home, '--with-colons', '--fingerprint', '--list-keys')
    require(result.returncode == 0, 'GnuPG key listing failed')
    fingerprints, primary = [], False
    for line in result.stdout.decode('utf-8').splitlines():
        fields = line.split(':')
        if fields[0] in ('pub', 'sub'):
            primary = fields[0] == 'pub'
        elif fields[0] == 'fpr' and primary:
            fingerprints.append(fields[9])
            primary = False
    return fingerprints


def pinned_key(home):
    import_key(home, PUBLIC_KEY.read_bytes())
    require(primary_fingerprints(home) == [PRIMARY_FINGERPRINT], 'committed release key primary fingerprint mismatch')
    secret = gpg(home, '--with-colons', '--list-secret-keys')
    require(secret.returncode == 0 and not any(line.startswith(b'sec:') for line in secret.stdout.splitlines()),
            'verification home must contain only the committed public key')


def valid_signature(status, returncode, primary):
    rows = [line.removeprefix('[GNUPG:] ').split() for line in status.decode('utf-8').splitlines() if line.startswith('[GNUPG:] ')]
    bad = {'BADSIG','ERRSIG','NO_PUBKEY','REVKEYSIG','FAILURE','NODATA'}
    signatures = [r for r in rows if r and r[0] == 'VALIDSIG']
    require(returncode == 0 and not any(r and r[0] in bad for r in rows) and len(signatures) == 1,
            'OpenPGP signature verification failed')
    row = signatures[0]
    require(len(row) in (10,11) and re.fullmatch('[0-9A-F]{40}', row[1]), 'malformed GnuPG VALIDSIG')
    actual_primary = row[10] if len(row) == 11 else row[1]
    require(actual_primary == primary, 'OpenPGP signature primary fingerprint mismatch')
    return dict(primary_fingerprint=actual_primary, signing_fingerprint=row[1], signature_created=int(row[3]))


def _verify_loaded(home, manifest, signature, primary):
    require(signature.is_file() and not signature.is_symlink() and 0 < signature.stat().st_size <= MAX_SIGNATURE,
            'missing, linked or oversized OpenPGP signature')
    result = gpg(home, '--status-fd', '1', '--verify', str(signature.resolve()), str(manifest.resolve()))
    return valid_signature(result.stdout, result.returncode, primary)


def verify(manifest, signature):
    """Always use the committed key and primary pin in an isolated public-only home."""
    with temporary_home() as home:
        pinned_key(home)
        return _verify_loaded(home, manifest, signature, PRIMARY_FINGERPRINT)


def _detach_sign(home, manifest, destination, subkey, passphrase):
    # Private primitive shared with ephemeral-key tests. No CLI exposes its key argument.
    require('\n' not in passphrase and '\r' not in passphrase, 'signing passphrase must be one line')
    result = gpg(home, '--pinentry-mode', 'loopback', '--passphrase-fd', '0', '--local-user', subkey + '!',
                 '--armor', '--detach-sign', '--output', str(destination.resolve()), str(manifest.resolve()),
                 data=(passphrase + '\n').encode())
    require(result.returncode == 0, 'OpenPGP checksum signing failed')


def _retain_first(home, manifest, destination, previous, primary):
    if previous is None:
        return False
    require(not previous.is_symlink(), 'prior signature is a link')
    if not previous.exists():
        return False
    _verify_loaded(home, manifest, previous, primary)
    destination.write_bytes(previous.read_bytes())
    return True


def sign(manifest, signing_home, verification_home, previous=None):
    """Sign before publishing, verify separately, and retain the first valid run signature."""
    empty_home(signing_home)
    empty_home(verification_home)
    require(signing_home.resolve() != verification_home.resolve(), 'signing and verification homes must differ')
    key = os.environ.pop('RELEASE_SIGNING_KEY', '')
    passphrase = os.environ.pop('RELEASE_SIGNING_PASSPHRASE', None)
    require(bool(key) and passphrase is not None, 'release signing environment secrets are required')
    import_key(signing_home, key.encode())
    require(primary_fingerprints(signing_home) == [PRIMARY_FINGERPRINT], 'secret release key primary fingerprint mismatch')
    destination = manifest.with_name(SIGNATURE)
    require(not destination.exists() and not destination.is_symlink(), 'signature output already exists; refusing overwrite')
    try:
        _detach_sign(signing_home, manifest, destination, SIGNING_SUBKEY, passphrase)
        pinned_key(verification_home)
        _verify_loaded(verification_home, manifest, destination, PRIMARY_FINGERPRINT)
        return _retain_first(verification_home, manifest, destination, previous, PRIMARY_FINGERPRINT)
    except BaseException:
        destination.unlink(missing_ok=True)
        raise


def check_all(manifest, expected_names):
    lines = manifest.read_text(encoding='utf-8').splitlines()
    require(all(re.fullmatch(r'[0-9a-f]{64}  [A-Za-z0-9_.-]+', line) for line in lines), 'invalid checksum manifest')
    names = [line.split('  ')[1] for line in lines]
    require(names == sorted(expected_names) and len(names) == len(set(names)), 'checksum names/order/duplicates mismatch')
    subprocess.run([tool('sha256sum'), '--check', '--strict', manifest.name], cwd=manifest.parent,
                   env=environment(), check=True, timeout=120)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    signing = commands.add_parser('sign')
    signing.add_argument('--manifest', type=Path, required=True)
    signing.add_argument('--sign-home', type=Path, required=True)
    signing.add_argument('--verify-home', type=Path, required=True)
    signing.add_argument('--previous', type=Path)
    signing.add_argument('--github-output', type=Path, required=True)
    checking = commands.add_parser('verify')
    checking.add_argument('--manifest', type=Path, required=True)
    checking.add_argument('--signature', type=Path, required=True)
    args = parser.parse_args()
    if args.command == 'sign':
        reused = sign(args.manifest, args.sign_home, args.verify_home, args.previous)
        with args.github_output.open('a') as output:
            output.write(f'reused={str(reused).lower()}\n')
    else:
        print(verify(args.manifest, args.signature))


if __name__ == '__main__':
    main()
