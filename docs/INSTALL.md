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
starts with 0.5.2; it is not added to v0.5.1. Version 0.5.3 is crate-only:
its failed SBOM attestation uploaded no prebuilt downloads. Version 0.5.4 is the
first prebuilt profile with the 0.5.3 security fixes and adds a Cargo SBOM to the
five archives, covered by one six-entry SHA256SUMS. Prefer 0.5.4 or newer downloads;
0.5.1/0.5.2 downloads retain their original, unpatched dependency graph.

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

From 0.5.4, download the archive, rust-mirror-VERSION.cdx.json and SHA256SUMS from
one release. The manifest has six sorted entries: five archives plus the SBOM.
v0.5.1 retains four entries and v0.5.2 five; those historical releases retain their
original release-binaries.yml signer and have no SBOM retrofit.

These examples pin 0.5.4 and use a recent [GitHub CLI](https://cli.github.com/manual/gh_attestation_verify).
Obtain the commit from the protected annotated tag through the GitHub API and
confirm it against the merged release PR before running a download. Do not take
an expected source commit from an unverified archive. The reusable signer and
source must both identify that exact commit; self-hosted runners are refused.

Windows PowerShell:

```powershell
$tag = 'v0.5.4'
$archive = 'rust-mirror-0.5.4-x86_64-pc-windows-msvc.zip'
$sbom = 'rust-mirror-0.5.4.cdx.json'
$tagObject = gh api "repos/rustcommons/rust-mirror/git/ref/tags/$tag" --jq '.object.sha'
if ($LASTEXITCODE -ne 0) { throw 'Tag lookup failed' }
$commit = gh api "repos/rustcommons/rust-mirror/git/tags/$tagObject" --jq '.object.sha'
if ($LASTEXITCODE -ne 0 -or $commit -cnotmatch '^[0-9a-f]{40}$') { throw 'Annotated tag lookup failed' }
# Confirm $commit against the merged release PR before continuing.
$policy = @('--repo', 'rustcommons/rust-mirror', '--signer-workflow',
  'rustcommons/rust-mirror/.github/workflows/build-binaries.yml',
  '--source-digest', $commit, '--signer-digest', $commit, '--deny-self-hosted-runners')
gh release download $tag --repo rustcommons/rust-mirror --pattern $archive --pattern $sbom --pattern SHA256SUMS
if ($LASTEXITCODE -ne 0) { throw 'Release download failed' }
foreach ($file in @($archive, $sbom)) {
  $lines = @(Get-Content .\SHA256SUMS | Where-Object { $_ -cmatch "^[0-9a-f]{64}  $([regex]::Escape($file))$" })
  if ($lines.Count -ne 1) { throw 'Missing or duplicate checksum entry' }
  $actual = (Get-FileHash -LiteralPath ".\$file" -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actual -cne $lines[0].Substring(0, 64)) { throw 'Checksum mismatch' }
}
foreach ($file in @($archive, $sbom, 'SHA256SUMS')) {
  gh attestation verify $file @policy --predicate-type https://slsa.dev/provenance/v1
  if ($LASTEXITCODE -ne 0) { throw 'Provenance verification failed' }
}
gh attestation verify $archive @policy --predicate-type https://cyclonedx.org/bom
if ($LASTEXITCODE -ne 0) { throw 'Archive SBOM attestation verification failed' }
Expand-Archive -LiteralPath ".\$archive" -DestinationPath .\rust-mirror-download
.\rust-mirror-download\rust-mirror-0.5.4-x86_64-pc-windows-msvc\rust-mirror.exe --version
```

Unix shell (Linux example; choose an Apple target on macOS):

```sh
set -eu
tag=v0.5.4
archive=rust-mirror-0.5.4-x86_64-unknown-linux-musl.tar.gz
sbom=rust-mirror-0.5.4.cdx.json
tag_object=$(gh api "repos/rustcommons/rust-mirror/git/ref/tags/$tag" --jq '.object.sha')
commit=$(gh api "repos/rustcommons/rust-mirror/git/tags/$tag_object" --jq '.object.sha')
printf '%s\n' "$commit" | grep -E '^[0-9a-f]{40}$'
# Confirm $commit against the merged release PR before continuing.
gh release download "$tag" --repo rustcommons/rust-mirror --pattern "$archive" --pattern "$sbom" --pattern SHA256SUMS
awk -v a="$archive" -v s="$sbom" '$2==a {x++; print} $2==s {y++; print} END {if (x!=1 || y!=1) exit 1}' SHA256SUMS > selected-payloads.sha256
shasum -a 256 -c selected-payloads.sha256
# Linux alternative: sha256sum --check selected-payloads.sha256
verify() {
  gh attestation verify "$1" --repo rustcommons/rust-mirror \
    --signer-workflow rustcommons/rust-mirror/.github/workflows/build-binaries.yml \
    --source-digest "$commit" --signer-digest "$commit" --deny-self-hosted-runners \
    --predicate-type "$2"
}
verify "$archive" https://slsa.dev/provenance/v1
verify "$sbom" https://slsa.dev/provenance/v1
verify SHA256SUMS https://slsa.dev/provenance/v1
verify "$archive" https://cyclonedx.org/bom
tar -xzf "$archive"
./rust-mirror-0.5.4-x86_64-unknown-linux-musl/rust-mirror --version
```

The read-only [public-download workflow](../.github/workflows/verify-release-binaries.yml),
dispatched with tag v0.5.4 after publication, additionally compares the exact signed
SBOM predicate with the downloaded document and the selected archive digest. It
checks every asset's provenance and source/lock binding before native execution.
The manual commands above verify the typed signed attestations; they do not perform
that full JSON-document comparison.

Checksums alone do not authenticate a download. Provenance does not imply OS code
signing, independent review or universal identical rebuilds. Historical dispatch
backfills identify their original workflow revision separately from product source;
do not apply the new same-commit signer policy to those old profiles. See
[release security automation](SECURITY-BASELINE.md#sbom-and-release-assets) for scope.

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
xattr -d com.apple.quarantine ./rust-mirror-0.5.4-aarch64-apple-darwin/rust-mirror
./rust-mirror-0.5.4-aarch64-apple-darwin/rust-mirror --version
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
cargo install rust-mirror --version 0.5.4 --locked
```

Cargo installs the executable into its configured binary directory (normally
`~/.cargo/bin` or `%USERPROFILE%\.cargo\bin`). Ensure it is on PATH and check
`rust-mirror --version`. This compiles locally; it does not download a GitHub
prebuilt archive. See the [user guide](USER-GUIDE.md) for operation and
[release policy](RELEASING.md#prebuilt-github-release-binaries) for build/asset rules.
