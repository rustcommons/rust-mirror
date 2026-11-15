# rust-mirror 0.5.1 publication — 2026-11-15

## Source and owner publication

The owner authorized qualified 0.5.1 publication. The coordinator normal-merged
[PR #41](https://github.com/rustcommons/rust-mirror/pull/41) at
`112ef7a7a71d40349c601f299f965199b5b279ba`, tree-identical to reviewed
`3eae26617dc07a4b6f0cf7b6df7658740ac446c6`; the empty source diff was checked again.
They reported publication from a clean worktree on the owner's Windows machine,
Cargo 1.99.0 x86_64-pc-windows-msvc, `cargo publish --locked`.
No credential was transferred for verification. The GUI maintainer was notified
that 0.5.1 is live.

The [candidate evidence](2026-11-15-index-collisions.md), paused runs and initial
failures remain historical. The qualified Linux/aarch64 Cargo 1.90 candidate
archive SHA-256 was
`b8a7c6974bcdae3585aeef0ff4f9740fd63b53b3e62344f9d3771881db838095`.
The published Windows archive SHA-256 is
`29b547bb8b762beb0f403dda6f8f628ab7706a18ccd52bb0f01711382a0bda39`.
These distinct archives are not interchangeable checksums.

## Official registry payload

Downloaded the official static.crates.io archive. Its hash matches both the
crates.io version API and sparse index; both identify 0.5.1 as non-yanked.
All 45 archive members are regular files. All 43 original source payloads,
including Cargo.toml.orig, lock, README, changelog and license, match the exact
merge byte-for-byte. Generated normalized Cargo.toml also matches the candidate;
only VCS file content differs among archive members, naming the clean merge.
This observation does not guarantee identical packaging across Cargo versions.
[Machine-readable verification](publication-0.5.1.json) preserves hashes/results.

## Tag, release and public documentation

Annotated `v0.5.1` points to the exact merge; the live remote tag and peeled commit
were checked. The [GitHub Release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.5.1)
is latest, public, non-prerelease, has no assets and matches committed versioned
notes byte-for-byte. The committed release-note body and published package were
not rewritten. Tag/release documentation runs
[43668168335](https://github.com/rustcommons/rust-mirror/actions/runs/43668168335) and
[43668169548](https://github.com/rustcommons/rust-mirror/actions/runs/43668169548)
passed, as did local full `--published` validation.

Exact versioned docs.rs URLs returned 200 without redirect fallback:
[API](https://docs.rs/rust-mirror/0.5.1/rust_mirror/),
[IndexWarning](https://docs.rs/rust-mirror/0.5.1/rust_mirror/crates/struct.IndexWarning.html),
[IndexWarningKind](https://docs.rs/rust-mirror/0.5.1/rust_mirror/crates/enum.IndexWarningKind.html),
[CollisionVersion](https://docs.rs/rust-mirror/0.5.1/rust_mirror/crates/struct.CollisionVersion.html)
and [events](https://docs.rs/rust-mirror/0.5.1/rust_mirror/events/).
The builds page identifies rustc 1.102.0-nightly (5f4effd57, 2026-11-13);
release qualification remains Rust/Cargo 1.90.

## Fresh actual registry installation

Ran `cargo install rust-mirror --version 0.5.1 --locked` with a new Cargo home and
install root, Cargo 1.90 on Linux/aarch64. Its downloaded archive matches the
verified official checksum. The installed binary reports 0.5.1 / protocol 1.
An initial shell helper attempted log redirection before its parallel directory
creation finished; Cargo did not start. The corrected fresh installation passed.

The actual installed CLI planned ndk-sys 0.6.0+11769913: eight versions and a typed
warning identifying the historical 0.4.0 pair. Local `crates show` revalidated the
frozen plan with the same plan ID, warnings and version/checksum identities.
Selecting 0.4.0 refused as `invalid` with `report.kind=index_version_collision`,
crate name, both full versions and checksums. An initial proof-helper assertion
compared saved JSON and event JSON literally: the latter omits null optional
source digests. Corrected assertions compare the validated plan ID and named
semantic fields; the commands passed. No product change was needed.

The installed CLI then completed preview/approval/sync for real itoa 1.0.15 and
served an isolated Cargo client over verified HTTPS: fetch --locked, build --locked
and populated offline run passed with unchanged lock. Untrusted CA, missing/corrupt
archive and fresh offline-home controls refused without upstream fallback.
This is one named crate client, not a framework build or arbitrary-project claim.
Fixture servers and temporary PKI directories were cleaned up; real Cargo/Rustup
homes were untouched.

## Workflow and bounded qualification

[Published registry verification](https://github.com/rustcommons/rust-mirror/actions/runs/43668169411)
passed from main while checking out immutable `v0.5.1`. Its downloaded artifact
confirms fresh registry installation, official checksum/clean merge VCS, protocol 1,
nine-file installed HTTPS publisher proof, typed TLS/auth/remote-exists refusals,
no-write preflight, empty network-namespace routes and private-key destruction.
This fixture is not actual Nexus; separate Nexus gates retain their named scope.

All [18 candidate checks](https://github.com/rustcommons/rust-mirror/pull/41/checks)
passed, including Cargo 1.90 collision conformance
[43666515600](https://github.com/rustcommons/rust-mirror/actions/runs/43666515600):
six full/filtered-index cases, zero archive requests, pinned real ndk-sys metadata,
yanked/live pairs, intentional selected-checksum refusal, Bevy-style target chain
and full-default real winit/eframe roots. The 397-version windowing plan is resolution
evidence, not builds of those frameworks or a full Bevy build. Original index lines
remain byte-preserved. Manual, lock-only and retained conflicts are covered.

Permanent gates also passed: top-100/three-OS Cargo clients/Linux Nexus
[43666515615](https://github.com/rustcommons/rust-mirror/actions/runs/43666515615),
Linux/Windows lock import [43666515490](https://github.com/rustcommons/rust-mirror/actions/runs/43666515490),
native Windows Nexus [43666515562](https://github.com/rustcommons/rust-mirror/actions/runs/43666515562)
and four-host Rustup lifecycle [43666515512](https://github.com/rustcommons/rust-mirror/actions/runs/43666515512).
Carry their bounded client/host/SDK/fixture evidence only. Historical-cohort and
unsupported-semantics refusals, resource bounds, old-reader warning-field refusal
and public struct-field upgrade requirements remain explicit in the release notes.
No arbitrary-catalog, production Nexus, GUI, independent-signature or redistribution
guarantee follows from publication.

Postmerge [main CI](https://github.com/rustcommons/rust-mirror/actions/runs/43667959859)
passed at the release merge. Main was clean and remote-aligned before this excluded-
documentation record branch. This PR changes no packaged source, dependency, version,
immutable tag/archive or release-note body and requires coordinator review before merge.

## Subsequent authorized binary backfill

The initial notes-only/no-assets observations above precede the owner's binary
distribution authorization. After the separately reviewed workflow merge,
[v0.5.1 binary backfill](2026-11-15-binary-backfill-0.5.1.md) attached four archives
and SHA256SUMS with verified provenance. The existing notes, source tag and crates.io
archive were unchanged. Preserve the initial publication record and its hashes.
