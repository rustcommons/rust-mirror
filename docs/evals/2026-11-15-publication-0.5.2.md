# rust-mirror 0.5.2 publication — 2026-11-15

## Source and owner publication

The owner authorized 0.5.2 for the five-download profile. The coordinator normally
merged [PR #47](https://github.com/rustcommons/rust-mirror/pull/47) at
`d9d4f16ba47dac26531bfbc8552e75aeaaeb09cb`, tree-identical to reviewed
`af9532e82d19c5a66d19f61eab2ed7f2fd0dbc72`; the empty diff was independently checked.
They reported `cargo publish --locked` from a clean detached worktree on the owner's
Windows machine, Cargo 1.99.0 x86_64-pc-windows-msvc. No credential was transferred.

Published archive SHA-256:
`3979d2425011f92f0d01e46bf6cba5f22b1c72cdbd2321b31ad8f66287369814`.
The official archive matches the crates.io API and sparse index, both non-yanked.
All 45 members are regular files; all 43 original payloads, including manifest
original, lock, README, changelog and license, match the merge byte-for-byte.
Normalized Cargo.toml matches the qualified candidate too; only VCS content differs,
naming the clean merge. This is a measured comparison, not a promise across Cargo
versions. The qualified Linux/aarch64 Cargo1.90 candidate hash remains
`9e9d201db6ebe9bdde6421a95a0d0578552451b7cacf7ac4c13936e64449e5c4`.
Do not substitute it for the distinct published archive checksum.

The [machine-readable record](publication-0.5.2.json) retains registry/source,
compiler, tag, documentation, binary hashes, userland gates and native execution.
Mirroring src/ and examples/ bytes and dependency identities match 0.5.1; the root
package/tool version and packaged user documentation change. Existing schema,
protocol, resource and unsupported-semantics limits remain.

## Tag, release and documentation

Annotated v0.5.2 object `90d7adfeafb06276dab621c72bbd1f5f496b3e17`
peels to the exact release merge; live remote references were checked. The public
[GitHub Release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.5.2)
is latest, non-prerelease, and its body matches committed versioned notes exactly.
Only the tag workflow attached assets. No existing asset, source tag or historical
release-note body was replaced. All four v0.5.1 archives and its original SHA256SUMS
still have their original names/hashes; no GNU backfill or second manifest exists.

An initial curl API request returned 403; the descriptive verification User-Agent
request succeeded and confirmed the checksum. Initial docs.rs checks returned 404;
fresh checks subsequently returned 200 at the exact versioned paths, first with
cache-busting queries and then directly without a query or redirect fallback:
[API](https://docs.rs/rust-mirror/0.5.2/rust_mirror/),
[events](https://docs.rs/rust-mirror/0.5.2/rust_mirror/events/) and
[crates](https://docs.rs/rust-mirror/0.5.2/rust_mirror/crates/).
The [build page](https://docs.rs/crate/rust-mirror/0.5.2/builds) identifies
rustc 1.102.0-nightly (5f4effd57, 2026-11-13). Release/MSRV qualification stays 1.90.

Tag and release documentation runs
[43744076565](https://github.com/rustcommons/rust-mirror/actions/runs/43744076565) and
[43744077743](https://github.com/rustcommons/rust-mirror/actions/runs/43744077743)
passed, as did local full --published validation for all eight versions.

## Five archives and provenance

[Tag-triggered build 43744076569](https://github.com/rustcommons/rust-mirror/actions/runs/43744076569)
passed from the exact merge with Rust1.90 and --locked. Every target passed original
and unpacked version/protocol smoke checks. Windows static CRT passed DLL-import
refusal; musl passed static ELF/ldd/empty-root checks, real DNS/TLS sync and private-CA
kill/cancel/resume. Both macOS architectures were built/smoked on native runners.

GNU used cargo-libc0.7.0 pinned to
e68ca7bf3a8ffd973c8576acc48abd0fbcc66d25, Clang18 and the qualified 2.17 method.
Actual maximum required GLIBC symbol is 2.17. All eight digest-pinned userlands
passed launch and real official HTTPS crate preview/approved sync: CentOS7, Alma8,
Leap42.3, Ubuntu16.04/18.04/20.04 and Debian8/9. The JSON binds all outcomes and
runtime library/image identities to the release source and binary hash. Initial
[40-combination measurements](2026-11-15-glibc-measurements.md) remain historical.

Downloaded all six public files, checked each GitHub asset digest, the exact
five-line sorted manifest, archive layout/source/license, and independently ran
`gh attestation verify` on every archive AND SHA256SUMS with signer workflow
rustcommons/rust-mirror/.github/workflows/release-binaries.yml. All passed.
Tag-triggered workflow and tagged source share the same merge identity.

| Asset | SHA-256 |
| --- | --- |
| rust-mirror-0.5.2-aarch64-apple-darwin.tar.gz | b0c6573c7374711afe5b2374a801cbbc8d989eb4bd051930c7f32cd11580d070 |
| rust-mirror-0.5.2-x86_64-apple-darwin.tar.gz | 36936a55e20df6922a04fbfa3ca97c53ff63bad07455be37a941ca34890d3775 |
| rust-mirror-0.5.2-x86_64-pc-windows-msvc.zip | cc28caa67ff34936de89f705e9ffb98c3c26f577f08ef4083100ae8e9f73ac01 |
| rust-mirror-0.5.2-x86_64-unknown-linux-gnu.tar.gz | 26343042404fdb4ba4250af152f8cfad5e8eae9d412d530854795e675f8730cd |
| rust-mirror-0.5.2-x86_64-unknown-linux-musl.tar.gz | 665a9a3d9218dcbd9179d5a9fd5fb355a29a798f178c331ce329c5ee37575b2e |
| SHA256SUMS | b32108940f4086c7633dc4e2a4171b934f70902936abf1473af8af824b392e95 |

## Actual downloaded Windows and Linux execution

[Public-download run 43744879432](https://github.com/rustcommons/rust-mirror/actions/runs/43744879432)
passed all three native jobs: Windows ZIP, GNU Linux and static musl Linux. Each
job downloaded and verified all six public digests/attestations before extracting
its target, verified source/license, and executed both --version and protocol-1
version discovery. Every downloaded binary reports 0.5.2. Product subprocesses
received no workflow/API credential variables.

Windows imported DLLs satisfy the static-CRT refusal policy. Downloaded GNU shows
maximum GLIBC2.17 and required dynamic runtimes; downloaded musl has no interpreter
or NEEDED libraries and is identified as static. Exact binary hashes/imports/ELF
results and event records are in JSON. This is execution of public release bytes,
not a Cargo rebuild or substitution of local build artifacts.

## Fresh registry installation and bounded proofs

An actual fresh `cargo install rust-mirror --version 0.5.2 --locked` completed with
Cargo1.90 on Linux/aarch64, new Cargo home and install root. Its downloaded archive
matches the official checksum. The installed binary reports 0.5.2/protocol1 and
completed real itoa1.0.15 preview/approval/sync. An isolated HTTPS Cargo client passed
fetch --locked, build --locked and populated offline run with unchanged lock.
Untrusted CA, missing/corrupt archive and fresh offline-home controls refused without
upstream fallback. Owned fixture servers/temp PKI were cleaned up; existing Cargo/Rustup homes were
not modified. This is one named crate, not arbitrary framework/project compilation.

[Registry verification 43744077546](https://github.com/rustcommons/rust-mirror/actions/runs/43744077546)
passed independently on Linux/x86-64 from immutable v0.5.2: actual fresh registry
installation, official checksum/clean-merge VCS, protocol1, installed HTTPS publisher
in a loopback-only namespace, TLS/auth/remote-exists refusals and private-key cleanup.
This fixture is not actual Nexus. The candidate's automatic crate, lock-import,
four-host Rustup/Linux Nexus and native Windows Nexus gates were all green in
[PR #47](https://github.com/rustcommons/rust-mirror/pull/47/checks); carry their exact
client/host/SDK/fixture bounds. Postmerge [main CI 43743861564](https://github.com/rustcommons/rust-mirror/actions/runs/43743861564)
was green, and main clean/remote-aligned before this excluded-documentation branch.

## Limits and record scope

GNU tests cover container userlands sharing a modern kernel; old native kernels
are untested. Leap42.3 is a community SLE-era proxy, not commercial SLES evidence.
For glibc below2.17 use static musl. Normal kernel/CPU/network/NSS limits remain.
Native macOS build smokes do not establish post-download execution or Gatekeeper UI;
Windows/Apple code signing/notarization remain absent. Provenance is separate from
OS signing, universal enterprise support, independent OpenPGP or production Nexus.

This record changes no packaged source/version/dependency, immutable tag/archive,
release-note body or uploaded asset. Historical candidate/404 observations and
prior publication outcomes are preserved. Coordinator review is required to merge
the record PR.
