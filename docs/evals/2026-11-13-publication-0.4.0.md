# rust-mirror 0.4.0 publication — 2026-11-13

## Source and owner publication

The owner authorized 0.4.0 on 2026-11-13. The coordinator normal-merged
[PR #34](https://github.com/rustcommons/rust-mirror/pull/34) at
`3f815fe3c698115937ba1b1071391c15258af3cc`, tree-identical to reviewed
`f864a4695d4a0dc29e8732f3f7146fcdc9785226`. They reported publication from a clean
detached worktree at that merge on the owner's Windows machine, Cargo 1.99.0
x86_64-pc-windows-msvc, `cargo publish --locked`, using the owner's local login.
No credential was transferred for verification here.

The [candidate record](2026-11-13-release-candidate-0.4.0.md) remains historical.
The published archive SHA-256 is
`c3af7381321b8dd1d780e6d519ca0a7933285165ad413bca55ce8a3efca96451`.
The final-head Linux/Cargo 1.90 candidate archive was
`6866353b403fd1de64d6561c1132aede5648046a35b5a32df9cf35e1c30c384b`;
the earlier qualified source archive/hash is preserved in its original record.
These are distinct archives, not interchangeable checksums.

## Registry payload

Downloaded the official static.crates.io archive. Its SHA-256 equals both the
crates.io version API and sparse-index checksum; both mark 0.4.0 non-yanked.
The archive has 40 regular-file members. All 38 original source members, including
Cargo.toml.orig, lock, README, changelog and license, match the merge byte-for-byte.
Generated normalized Cargo.toml also matches the Linux candidate byte-for-byte.
The sole differing member is `.cargo_vcs_info.json`: the registry archive names
the exact clean merge instead of the candidate commit. No source drift or generated
manifest difference was observed; do not infer that Cargo versions always produce
identical archives. [Machine-readable verification](publication-0.4.0.json) retains
member hashes, source identity and installed evidence.

## Public documentation and release

Created annotated `v0.4.0` at the exact merge and verified its peeled commit.
The [GitHub Release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.4.0)
is public, latest, non-prerelease, has no attached binaries/assets, and its body
matches the committed versioned notes byte-for-byte. Tag/release documentation
workflows [43421451469](https://github.com/rustcommons/rust-mirror/actions/runs/43421451469)
and [43421453786](https://github.com/rustcommons/rust-mirror/actions/runs/43421453786)
passed. Local full `--published` validation passed with authentication after the
initial anonymous GitHub API request returned 403; no release content was changed.

Initially docs.rs was still building: HTTP 200 responses redirected to the crate
landing page and were not accepted as rustdoc success. After completion, the
[0.4.0 API](https://docs.rs/rust-mirror/0.4.0/rust_mirror/), crate mirror/catalog and
events pages returned 200 at their exact rustdoc URLs without fallback redirects.
The builds page identifies rustc 1.102.0-nightly (8eb778d04, 2026-11-12); local and
CI qualification still use Rust/Cargo 1.90. The GUI maintainer
was notified immediately that the registry version is live; GUI Phase 2 remains
separately governed.

## Actual registry installation

Used a new Cargo home and install root for `cargo install rust-mirror --version
0.4.0 --locked`, Cargo 1.90 on Linux/aarch64. The downloaded install archive has the
same official checksum. The installed binary reported tool 0.4.0 / protocol 1,
then completed a real itoa 1.0.15 HTTPS Cargo fetch/build and offline run from
isolated homes, preserving the lockfile. Untrusted CA, missing/corrupt archives and
a fresh offline home were refused. TLS verification stayed enabled and temporary
fixture keys were removed; no real client homes or production endpoints changed.

## Qualification boundaries

The accepted [crate MVP evidence](2026-11-13-crates-mvp.md) and final candidate
[CI gates](https://github.com/rustcommons/rust-mirror/pull/34/checks) remain bounded:
100 dated roots, 186-version native/Cargo union, 45,125,358 archive bytes each and
0% measured byte overfetch; zero planning archives. Client builds cover itoa,
not every corpus crate or arbitrary target. Actual disposable Nexus crate client
proof is Linux-only. Historical/current cohorts requiring unmirrored edges refuse
rather than expose incomplete output. No new completeness, production Nexus,
GUI, independent-signature or redistribution guarantee follows from publication.

## Registry workflow and main

The original main registry workflow
[43421453150](https://github.com/rustcommons/rust-mirror/actions/runs/43421453150)
passed. This record PR additionally applies the release CI's explicit fixture-key
cleanup to registry verification: delete only its two owned temporary keys on exit,
assert their absence and retain allowlisted proof logs. No published source changes.
The strengthened run
[43421710237](https://github.com/rustcommons/rust-mirror/actions/runs/43421710237)
passed using workflow revision `7efcde589333d94acfb0ec7edd16bf973adc0ed3` from
this branch, while checking out immutable `v0.4.0` for source/proof scripts.

Downloaded its artifact: actual fresh crates.io install, exact registry checksum and
clean merge VCS, protocol 1, nine-file HTTPS publisher fixture with typed
TLS/auth/remote-exists refusals and no-write preflight, empty network-namespace
routes, and explicit private-key destruction. This fixture is not actual Nexus.
The earlier actual Nexus proof retains its separate bounded identity.

Postmerge main CI
[43421173516](https://github.com/rustcommons/rust-mirror/actions/runs/43421173516)
passed on `3f815fe3c698115937ba1b1071391c15258af3cc`. Main was remote-aligned and
clean before this excluded-documentation/workflow branch. The publication-record PR
requires review; it does not alter the immutable registry archive or tag.
