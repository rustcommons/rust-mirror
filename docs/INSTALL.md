# Install rust-mirror

## Download a prebuilt binary

Choose the archive matching your computer from
[GitHub Releases](https://github.com/rustcommons/rust-mirror/releases/latest).
No Rust/Cargo installation is needed to run the CLI.

| Computer | Archive target | Format |
| --- | --- | --- |
| Windows, Intel/AMD 64-bit | x86_64-pc-windows-msvc | .zip |
| Linux glibc 2.17+ (0.5.2+) | x86_64-unknown-linux-gnu | .tar.gz |
| Any x86_64 Linux, fully static musl | x86_64-unknown-linux-musl | .tar.gz |
| macOS, Apple Silicon | aarch64-apple-darwin | .tar.gz |
| macOS, Intel | x86_64-apple-darwin | .tar.gz |

v0.5.1 keeps its four existing archives and original SHA256SUMS. The GNU archive
starts with 0.5.2; it is not added to v0.5.1. Releases from 0.5.2 have five
archives covered by one SHA256SUMS.

Choose **musl** to avoid dynamic libc/runtime dependencies across x86_64 Linux
userlands. Choose **GNU/glibc** when your environment prefers the standard dynamic
glibc ABI. The GNU variant targets **glibc 2.17** with pinned cargo-libc 0.7.0,
Rust 1.90 and Clang 18. Each release must pass launch and real HTTPS crate
preview/approved-sync checks in these digest-pinned container userlands:
CentOS 7, AlmaLinux 8, openSUSE Leap 42.3, Ubuntu 16.04/18.04/20.04 and Debian 8/9.
Its README records actual GLIBC symbol requirements and shared runtime libraries,
including libgcc_s. These are **container userland checks only** on a modern host
kernel; original old kernels, arbitrary NSS/SSO setups and native commercial SLES
are untested. For **glibc below 2.17, use musl**. See
[measured results](evals/2026-11-15-glibc-measurements.md) and
[the build policy](RELEASING.md#glibc-baseline-and-ci-only-compatibility-measurements).

Names use `rust-mirror-VERSION-TARGET`, without a `v` in VERSION. Each archive
contains one directory of that name with the executable, LICENSE and README.txt.
Put the executable in a directory on PATH or run it by its full path.
There is no installer, service or automatic updater.

Windows: nothing else to install: the C runtime is built in, so no Visual C++
Redistributable is needed. The release build's DLL audit refuses MSVC/UCRT runtime
imports; normal Windows system DLLs remain required. This applies to the release
archives, not independently compiled Cargo/development builds.

The musl Linux executable is fully static: no glibc, OpenSSL or other shared-library
runtime dependency. Its build gate checks file/ldd and ELF headers, refuses a dynamic
interpreter or NEEDED libraries, and runs it in an empty chroot. The musl binary
also performs real crate preview/approved sync over HTTPS and a private-CA proxy
resume/cancellation fixture. Normal Linux kernel/CPU/filesystem requirements still
apply; this is not execution evidence for every enterprise kernel/configuration.

DNS uses musl's resolver with /etc/hosts and /etc/resolv.conf; it does not load
glibc NSS modules such as SSSD/mDNS integrations. Corporate name resolution must
work through normal DNS or an explicitly configured proxy. TLS uses rustls and
bundled public roots, with corporate CA files added explicitly through
[network options](NETWORK.md); OpenSSL and a glibc trust-store plugin are not needed.
The [musl resolver reference](https://wiki.musl-libc.org/functional-differences-from-glibc.html)
describes differences from glibc. Named DNS/TLS smoke evidence is recorded with the
release qualification; it does not certify every proxy/SSO/NSS policy.

Windows and macOS builds are smoke-tested on their named hosted runners; this does
not establish every older OS version or native SDK combination.

### Verify before running

Download the archive and SHA256SUMS from the same release. SHA256SUMS has one sorted
line per archive (four for v0.5.1, five from 0.5.2): lowercase SHA-256, two spaces, file name. Verify the checksum and
[GitHub provenance](https://cli.github.com/manual/gh_attestation_verify), using a
recent GitHub CLI with artifact attestation support. These examples pin 0.5.2;
substitute another published version and your target as appropriate.

Windows PowerShell:

```powershell
$archive = 'rust-mirror-0.5.2-x86_64-pc-windows-msvc.zip'
gh release download v0.5.2 --repo rustcommons/rust-mirror --pattern $archive --pattern SHA256SUMS
if ($LASTEXITCODE -ne 0) { throw 'Release download failed' }
$lines = @(Get-Content .\SHA256SUMS | Where-Object { $_ -cmatch "^[0-9a-f]{64}  $([regex]::Escape($archive))$" })
if ($lines.Count -ne 1) { throw 'Missing or duplicate checksum entry' }
$expected = $lines[0].Substring(0, 64)
$actual = (Get-FileHash -LiteralPath ".\$archive" -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -cne $expected) { throw 'Checksum mismatch' }
gh attestation verify $archive --repo rustcommons/rust-mirror --signer-workflow rustcommons/rust-mirror/.github/workflows/release-binaries.yml
if ($LASTEXITCODE -ne 0) { throw 'Archive provenance verification failed' }
gh attestation verify SHA256SUMS --repo rustcommons/rust-mirror --signer-workflow rustcommons/rust-mirror/.github/workflows/release-binaries.yml
if ($LASTEXITCODE -ne 0) { throw 'Checksum provenance verification failed' }
Expand-Archive -LiteralPath ".\$archive" -DestinationPath .\rust-mirror-download
.\rust-mirror-download\rust-mirror-0.5.2-x86_64-pc-windows-msvc\rust-mirror.exe --version
```

Unix shell (Linux example; choose an Apple target on macOS):

```sh
set -eu
archive=rust-mirror-0.5.2-x86_64-unknown-linux-musl.tar.gz
gh release download v0.5.2 --repo rustcommons/rust-mirror --pattern "$archive" --pattern SHA256SUMS
grep -F "  $archive" SHA256SUMS > "$archive.sha256"
shasum -a 256 -c "$archive.sha256"
# Linux alternative: sha256sum --check "$archive.sha256"
gh attestation verify "$archive" --repo rustcommons/rust-mirror --signer-workflow rustcommons/rust-mirror/.github/workflows/release-binaries.yml
gh attestation verify SHA256SUMS --repo rustcommons/rust-mirror --signer-workflow rustcommons/rust-mirror/.github/workflows/release-binaries.yml
tar -xzf "$archive"
./rust-mirror-0.5.2-x86_64-unknown-linux-musl/rust-mirror --version
```

Attestations bind the downloaded file digest to this repository's release workflow.
For dispatch backfills, GitHub's provenance identifies the workflow revision on main;
the archive's attested README.txt binds the separately checked-out immutable source
tag and commit. For tag-triggered builds the workflow and source revisions coincide.
Checksums alone do not authenticate a download. Provenance does not imply OS code
signing, independent code review or identical rebuilds.

### Unsigned Windows and macOS downloads

These executables have no paid Windows Authenticode signature or Apple Developer ID
signature/notarization. GitHub/Sigstore signs provenance statements, not those OS
signatures. An Apple Silicon executable may have the linker's ad-hoc signature.

Windows SmartScreen may show **Windows protected your PC** or an unknown-publisher
prompt. After verifying the archive and provenance, select **More info**, then
**Run anyway**, if offered. If Windows marks the verified extracted executable as
blocked, its Properties dialog may offer **Unblock**. Do not turn off SmartScreen
globally. See [Microsoft's SmartScreen explanation](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation). If company policy prevents running it, ask your administrator.

macOS Gatekeeper may refuse an unidentified developer or an unnotarized download.
After verification, attempt to run the executable, then use **System Settings >
Privacy & Security > Open Anyway** for that item, if offered. Alternatively remove
only that verified executable's download quarantine flag, then retry:

```sh
xattr -d com.apple.quarantine ./rust-mirror-0.5.2-aarch64-apple-darwin/rust-mirror
./rust-mirror-0.5.2-aarch64-apple-darwin/rust-mirror --version
```

Use your actual extracted path. A missing quarantine attribute needs no removal.
Do not disable Gatekeeper globally. See Apple's
[opening an app from an unidentified developer](https://support.apple.com/en-us/102445)
for the platform's controls. No signing/notarization prompt was bypassed in CI;
runner smoke tests exercise freshly built executables, not downloaded quarantine UI.

## Install with Cargo

With Rust/Cargo 1.90 or newer:

```sh
cargo install rust-mirror --locked
# Pin a particular crate release when needed:
cargo install rust-mirror --version 0.5.2 --locked
```

Cargo installs the executable into its configured binary directory (normally
`~/.cargo/bin` or `%USERPROFILE%\.cargo\bin`). Ensure it is on PATH and check
`rust-mirror --version`. This compiles locally; it does not download a GitHub
prebuilt archive. See the [user guide](USER-GUIDE.md) for operation and
[release policy](RELEASING.md#prebuilt-github-release-binaries) for build/asset rules.
