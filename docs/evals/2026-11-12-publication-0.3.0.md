# rust-mirror 0.3.0 publication verification — 2026-11-12

## Owner action and immutable source

The coordinator reports owner-authorized publication from a clean detached Windows
worktree at normal merge `50bc1895eab5ed305a30ed6e44fafc2bc057e7a9`, using Cargo
1.99.0 (`x86_64-pc-windows-msvc`) and `cargo publish --locked`. The official registry
records upload at `2026-11-12T10:13:30.799403Z` UTC. The merge tree is identical to
qualified PR #28 head `7a00908bc5ad906391173bfc09ceaf546bd18b1c`; an independent
Git diff is empty. No source, lock, version or published package bytes were edited
by these postpublication checks.

The [exact frozen qualification record](https://github.com/rustcommons/rust-mirror/pull/28#issuecomment-6815519086)
records all nine passing checks, four native hosts, the clean installed package
and actual disposable Nexus schema 2/3. General CI run 43159993104 and native run
43159993086 remain bounded to that source/tree. macOS private keys were destroyed;
trust cleanup used the explicitly recorded disposable hosted-VM teardown policy
rather than claiming privileged certificate removal. Neither source nor fixture
results imply a corporate-production deployment or arbitrary proxy compatibility.

## Official archive and candidate comparison

Independent official archive/API/sparse-index inspection agrees on version 0.3.0,
non-yanked status and SHA-256:
`fcfd258dce7328ce54a5096b80f6b90e550642f9bb324baf262acaa1cecf11f9`.
`.cargo_vcs_info.json` names the exact clean normal merge. All 27 packaged
source/original-manifest/lock/README/changelog/license/example files match that
merge byte-for-byte, with 29 archive members including generated metadata.

The earlier clean Linux/Rust 1.90 candidate archive remains
`0b32c4b96aae9a5934d4da877c7d86ab0498e0266f36a7f75827407be7980e3b`,
naming the qualified PR head. All archived file contents, including generated
`Cargo.toml`, match the publication **except VCS metadata**, which properly names
the merge instead. Tar modification times differ for `.cargo_vcs_info.json`,
`Cargo.lock` and `Cargo.toml` (published `1153704088`, candidate `1`). Preserve both
archive identities; the comparison does not claim a generated-manifest content
change or silently replace the historical candidate checksum.

## Fresh registry install and installed-command proof

[Registry verification run 43198654165](https://github.com/rustcommons/rust-mirror/actions/runs/43198654165)
passed on Linux/x86-64 with Rust/Cargo 1.90. A fresh Cargo home installed the actual
registry crate using `cargo install rust-mirror --version 0.3.0 --locked`.
Its downloaded checksum/VCS agree with the API and annotated source tag. Installed
binary SHA-256: `67b85bb72e0d9befb5d1c2d99beb02e07dc995961e92ef4240e5854f082816de`.
Installed `--events jsonl --version` reports tool 0.3.0/protocol 1 with sequenced
hello/stage/final-result framing.

The installed publisher passed the existing loopback-only namespace fixture: nine
files, private-CA HTTPS, untrusted CA/wrong hostname refusals, typed TLS/auth/
remote-exists outcomes and no-PUT preflight. This is installed HTTP/TLS-fixture
evidence, distinct from the prepublication actual Nexus proof. It is not a new
production write, remote prune or unrelated four-host requalification. Uploaded
artifacts contain bounded public identities and proof logs, not fixture private keys.

## Tag, release and published documentation

- Annotated immutable [v0.3.0](https://github.com/rustcommons/rust-mirror/tree/v0.3.0)
  peels to `50bc1895eab5ed305a30ed6e44fafc2bc057e7a9`.
- [GitHub Release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.3.0)
  is latest, published/non-prerelease, with the exact committed
  [versioned notes](../releases/v0.3.0.md) and no attached distribution assets.
- [Full published-documentation run 43198659048](https://github.com/rustcommons/rust-mirror/actions/runs/43198659048)
  and local `check-release-docs.py --published` passed. Live release/tag/body/latest
  agreement was checked independently.
- The first postpublication docs.rs check returned 404 for 0.3.0 while the public
  build queue listed the crate. This observation is preserved, not represented as
  a failed source build. The queue subsequently cleared and all checked versioned
  pages returned HTTP 200 with generated API content: [crate documentation](https://docs.rs/rust-mirror/0.3.0/rust_mirror/),
  [network APIs](https://docs.rs/rust-mirror/0.3.0/rust_mirror/network/),
  [NetworkOptions](https://docs.rs/rust-mirror/0.3.0/rust_mirror/network/struct.NetworkOptions.html)
  and [release discovery](https://docs.rs/rust-mirror/0.3.0/rust_mirror/releases/).
  [crates.io](https://crates.io/crates/rust-mirror/0.3.0) serves the concise packaged README.

Machine-readable records: [archive/payload](0.3.0-payload-verification.json),
[registry-installed identity](0.3.0-registry-identity.json),
[tag/release](0.3.0-github-release-verification.json),
[initial docs.rs queue/404 observations](0.3.0-initial-docs-observations.json) and
[resolved published pages](0.3.0-published-pages.json).

The GUI maintainer was notified immediately after the owner's upload report and
again after independent verification. No second upload, tag move, standalone binary
release or production Nexus mutation was needed. G5 exact resume-state inspection,
SOCKS/IPv6-literal proxy endpoints, NTLM/SSO, in-generation compaction, private-workspace
cleanup and Nexus pruning remain planned/deferred as documented. Displayed certificate
metadata does not change trust; socket deadlines do not bound the OS DNS resolver.
