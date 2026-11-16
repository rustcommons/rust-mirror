# rust-mirror 0.5.4 publication — 2026-11-16

## Immutable source and Trusted Publishing

The owner authorized 0.5.4 and rejected a different-builder recovery for 0.5.3.
The coordinator normally merged [PR #63](https://github.com/rustcommons/rust-mirror/pull/63)
at `b254027cd8a31ebadd92f712671ed0d76c28561a`, tree-identical to qualified
`8afa238ba13c7810e6c2829c7e34471c0b19d34a`. After the owner's crates-io environment
approval, [Trusted Publishing 43807377829](https://github.com/rustcommons/rust-mirror/actions/runs/43807377829)
succeeded. Official crates.io trustpub_data independently identifies github,
rustcommons/rust-mirror, that run and that exact merge; registry creation time is
2026-11-16T05:03:22.470151Z. No long-lived Cargo token was used.

Published archive SHA-256:
`a11f6b58f2004b6df41ea1c33720bed7762c9daff815ce2eb3c3195fbbe908f2`.
API, sparse index, downloaded archive and the clean merge package agree; non-yanked.
All 45 members are regular files; all 43 original Git payloads match the merge.
Normalized Cargo.toml matches the qualified candidate; only .cargo_vcs_info.json
changes between candidate and published payload, naming the clean merge. The
Cargo 1.90 Linux/aarch64 candidate checksum remains
`2f3c2b68c33765df2e06f4ebd490e3ec453e5c9c6febd7d9124bf2e290bb4b24`.
Do not substitute it for the published checksum.

Annotated v0.5.4 object `3b99876b897704e0c382c2dcb7aa37a1e4c69363` peels to the
merge locally and remotely. The public, latest, non-prerelease
[GitHub Release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.5.4)
body matches the tagged versioned notes exactly. No collector retry was needed:
publication was confirmed and the empty exact-note release created before the
collector reached its bounded wait. No source/signer exception or tag move occurred.

The [machine-readable record](publication-0.5.4.json) retains source, registry,
package, run, asset, SBOM, native and documentation evidence. Application Rust and
example bytes/dependency identities match 0.5.3 except the root package version;
[preparation evidence](2026-11-16-release-0.5.4.md) identifies that comparison.
The release tooling carries the merged deterministic SBOM serial fix.

## Seven live files and reusable provenance

[Tag build 43807377864](https://github.com/rustcommons/rust-mirror/actions/runs/43807377864)
passed all five native builds, original/unpacked smokes, final SBOM generation,
provenance, archive/SBOM binding verification and append-only upload. Windows uses
static CRT and /Brepro with external-CRT import refusal. Musl passed static ELF,
empty-root, real DNS/TLS plan/sync and private-CA kill/cancel/resume controls. Both
macOS architectures passed native build/unpacked smokes. GNU uses pinned cargo-libc
0.7.0/Clang 18 targeting glibc 2.17; all eight digest-pinned userland launch/real HTTPS
preview/approved-sync gates passed: CentOS 7, AlmaLinux 8, Leap 42.3,
Ubuntu 16.04/18.04/20.04 and Debian 8/9. JSON binds the report to the published binary.

Downloaded all seven public files. Each GitHub asset digest matches local SHA-256;
SHA256SUMS has exactly six sorted entries covering five archives and the SBOM,
excluding itself. Archive layout, source-bound README.txt and original LICENSE
match the tag. The table below lists published assets, not candidate artifacts.

| Asset | SHA-256 |
| --- | --- |
| SHA256SUMS | d0d39b2c8b21bee5d6fe50167752c7b7e80a2286dfcc820b87a81d05a9fab0af |
| rust-mirror-0.5.4-aarch64-apple-darwin.tar.gz | 47093583da0179d09fd8c85342885e3dc5a6d32b0988bb509aac83b741ec0623 |
| rust-mirror-0.5.4-x86_64-apple-darwin.tar.gz | 08e6a62f2a7fc23426d95d655b2d29db510701eee364b3fd9b9abbd2c6a42a2a |
| rust-mirror-0.5.4-x86_64-pc-windows-msvc.zip | 74f1ff5af14866c95c1cca86e1e04d93c8ef19ff1f93a325dae6730eb086ad1d |
| rust-mirror-0.5.4-x86_64-unknown-linux-gnu.tar.gz | d1806bc1cbe8a95e125328f8f3cb3f053b2bdf8d8e4389a7ac843a42559b6228 |
| rust-mirror-0.5.4-x86_64-unknown-linux-musl.tar.gz | fd2476364b104bac1c9898e7d172f6cd5e3872f3874feab479b05cf9528cd74d |
| rust-mirror-0.5.4.cdx.json | 2265bac32b1393d9c96208696c629544b5ea9055562548a787b3b45e6b16b670 |

Independently verified SLSA provenance for **each of the seven files** with signer
rustcommons/rust-mirror/.github/workflows/build-binaries.yml, exact source digest
and signer digest b254027cd8a31ebadd92f712671ed0d76c28561a, and
--deny-self-hosted-runners. Also verified the CycloneDX predicate for **all five**
archives: each exact signed document equals the downloaded SBOM and names that
archive's SHA-256. This includes the macOS archives, beyond the three native
post-download execution targets.

The SBOM root is rust-mirror 0.5.4, CycloneDX 1.5 with a valid deterministic UUID.
Its source commit/lock properties and all 127 registry component checksums match
the tagged Cargo.lock. Scope is the all-target Cargo graph, not OS runtimes, SDKs
or CI tools. Raw verified attestations are workflow artifacts; JSON records the
policy and results without embedding large signature bundles.

The isolated reusable build/sign workflow and verified signer follow
[GitHub's SLSA v1 Build Level 3 design](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/increase-security-rating).
The current README's new L3 badge links this release-specific evidence after live
verification; the immutable published crate README is unchanged. This is build
provenance, not independent security certification, OS signing or proof that all
source/dependencies are harmless. Historical signers remain historical.

Candidate [paired measurement 43805637147](https://github.com/rustcommons/rust-mirror/actions/runs/43805637147)
found identical executable/archive pairs for all five targets and identical SBOMs
for two independent clean builds at 8afa238. The dry-run SBOM matched that paired
result. These measurements are candidate-specific, not two production dispatches
or a universal reproducibility guarantee; production README/SBOM source identities
name the merge instead.

## Actual public Windows, GNU and musl execution

Initial [public run 43808394284](https://github.com/rustcommons/rust-mirror/actions/runs/43808394284)
passed GNU/musl but refused Windows before binary execution. The read-only verifier
used Windows' CP1252 default to decode GitHub CLI UTF-8 JSON. A non-ASCII dependency
author value then differed from the correctly parsed SBOM. Replaying the actual
published statement passes exact binding under UTF-8 and refuses under CP1252;
the diagnostic records only the differing field path, not author identifiers.
No archive, signature, producer, package, dependency or protected tag was defective.

The verifier now specifies UTF-8 for subprocess JSON and proof/document files.
A regression emits real UTF-8 JSON under a simulated ANSI locale and requires exact
binding, while the corrupted predicate must still refuse. There is no Unicode
normalization, skipped comparison or digest/source/signer relaxation. The read-only
workflow default also advances to the current v0.5.4 profile.

Corrected [run 43808915816](https://github.com/rustcommons/rust-mirror/actions/runs/43808915816)
passed all three targets from verifier revision
`c9f2a807a9a433cf4e4a4c12d5014ecb07784ff5`. This reader revision is distinct from
immutable application/producer source b254027; verifying old releases with a newer
reader is the existing read-only contract. All seven digests/provenances and the
selected exact SBOM binding were checked before each job extracted and executed
its actual public Windows ZIP, GNU or musl download. Version/protocol discovery
reported 0.5.4/protocol1. Windows DLL imports passed static-CRT refusal; GNU measured
maximum required GLIBC 2.17 and musl has no dynamic interpreter/NEEDED libraries.
Product subprocesses received no workflow/API credentials. Exact binary hashes,
imports/ELF findings and events are in JSON. Neither old files nor the initial
failed run were deleted or presented as a pass.

## Registry installation and public documentation

A fresh actual cargo install rust-mirror --version 0.5.4 --locked passed on
Linux/aarch64 with Cargo 1.90, a new Cargo home/target/install root, official checksum
and clean merge VCS. The installed binary passed version/protocol discovery, typed
I/O refusal and real itoa1.0.15 plan/whole-preview-approved fetch. This does not
claim a new arbitrary Cargo client build or actual Nexus.

Independently, [registry run 43807954825](https://github.com/rustcommons/rust-mirror/actions/runs/43807954825)
passed Linux/x86-64 fresh installation, official archive/VCS checks, protocol,
installed publisher in a loopback-only network namespace, TLS/auth/remote-exists
controls and private-key destruction. The HTTPS namespace fixture is not Nexus.
The release PR's native/client/Nexus/conformance gates were green within their
named host/client/SDK bounds; no unrelated full matrix was repeated for this record.

At 05:05 UTC the four versioned docs.rs requests returned 404. At 05:09 UTC all
returned 200 directly, containing 0.5.4, with no older-version redirect fallback:
[API](https://docs.rs/rust-mirror/0.5.4/rust_mirror/),
[events](https://docs.rs/rust-mirror/0.5.4/rust_mirror/events/),
[crates](https://docs.rs/rust-mirror/0.5.4/rust_mirror/crates/) and
[lockfile](https://docs.rs/rust-mirror/0.5.4/rust_mirror/crates/lockfile/).
Both observations remain in JSON. All-ten-version published documentation checks
passed locally and in [43807957584](https://github.com/rustcommons/rust-mirror/actions/runs/43807957584).
Main merge CI [43807324034](https://github.com/rustcommons/rust-mirror/actions/runs/43807324034)
and security [43807324067](https://github.com/rustcommons/rust-mirror/actions/runs/43807324067)
passed at b254027 before this excluded-record branch.

## Advisory, immutable history and limits

The repository advisory GHSA-46c8-w6pp-97pw remains **draft**, unpublished, for the
coordinator. Its description now identifies verified available 0.5.4 downloads,
affected <=0.5.2 and patched crate 0.5.3. This is the first download profile carrying
those inherited dependency fixes, not a new application vulnerability fix.
The [release security discussion](../releases/v0.5.4.md#security) preserves the
complete upstream source/exposure bounds and absence of a demonstrated exploit.

Compared saved before/after snapshots for v0.5.1/v0.5.2/v0.5.3: release IDs, exact
note-body hashes, asset IDs/sizes/digests are unchanged. Version 0.5.3 remains crate-only
with zero assets and both historical failures. No backfill, second manifest, source/
signer exception, asset overwrite or historical note-body edit occurred.

GNU results cover container userlands sharing modern hosted kernels, not old native
kernels or commercial SLES. For glibc below 2.17 use musl; normal CPU/kernel/network/
NSS limits remain. macOS build smokes and cryptographic archive verification do not
establish public macOS execution or Gatekeeper UI. OS signing/notarization remain
absent. Independent OpenPGP, hostile-filesystem/NFS, arbitrary SDK/client builds,
production Nexus and globally atomic activation are not certified. This record
changes only documentation and the read-only verifier/default/locale regression;
it changes no application Rust implementation, dependency/version, producer,
immutable archive, tag, GitHub note body or published release asset.
