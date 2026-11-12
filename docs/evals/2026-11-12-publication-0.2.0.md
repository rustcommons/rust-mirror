# rust-mirror 0.2.0 publication verification — 2026-11-12

## Source and publication

The coordinator reports owner-authorized publication from a clean detached Windows
worktree at `9cdf22f58ab61e894bebf07ffc2bdaa8e3b5de45`, using Cargo 1.99.0
(`x86_64-pc-windows-msvc`) and `cargo publish --locked`. The registry records upload
at `2026-11-12T02:44:16.945073Z` UTC. This records actual UTC publication, separately
from the 2026-11-11 authorization, changelog and committed release-note date.

Independent official archive/API/sparse-index inspection verifies version 0.2.0,
non-yanked status and matching SHA-256:
`6674adce8e30a48b4586c972fa0ef628b69da9f21fc023f2be892435abcd2358`.
`.cargo_vcs_info.json` names the exact clean release merge above. All 24 packaged
source/original-manifest/lock/README/changelog/license/example files match that
commit byte-for-byte. The published archive and the previously qualified Linux
archive have identical membership and every file's contents, including generated
`Cargo.toml` and VCS metadata. Tar member modification times differ for
`.cargo_vcs_info.json`, `Cargo.lock` and `Cargo.toml` (published `1153704088`, Linux
`1`); tar/container bytes and archive checksums therefore differ. The observed
comparison does not require or claim a generated-manifest content change.

The earlier Linux/Rust 1.90 archive hash
`d7da9df9d0f824bcf5703a551ca5c783a2b04f2c751706d6b934cff934823653`
and [prepublication qualification](2026-11-11-events-020.md) remain historical
facts. Neither archive identity is rewritten to match the other. Qualified runtime
and dependency bytes are unchanged; no new native matrix is inferred from metadata.

## Registry installation and installed-command proof

[Published registry verification run 43143958780](https://github.com/rustcommons/rust-mirror/actions/runs/43143958780)
passed with Rust/Cargo 1.90 on Linux/x86-64. A fresh Cargo home installed the actual
registry crate using `cargo install rust-mirror --version 0.2.0 --locked`.
Its downloaded archive matches the official checksum and tag source. The installed
binary SHA-256 is `cf120bf9b70819a43d128f2171841fa10d12798518914fd1c5a1a2a5c3aa2bab`.
Cheap local `--events jsonl --version` reports tool 0.2.0/protocol 1 with framed,
sequenced hello/stage/result events.

The installed binary passed the established loopback-only network-namespace
publisher fixture: nine files, trusted private CA, untrusted CA and wrong-hostname
refusals, typed TLS/auth/remote-exists outcomes and no-PUT preflight. This is an
isolated HTTP/TLS fixture, not additional actual Nexus evidence. Actual disposable
Nexus and the four native hosts remain bounded to the linked prepublication run.
No production Nexus data was written or deleted. The workflow artifact contains
registry identity and bounded publisher logs; it excludes fixture private keys.

## Tag, release and published documentation

- Annotated immutable [v0.2.0](https://github.com/rustcommons/rust-mirror/tree/v0.2.0)
  peels to `9cdf22f58ab61e894bebf07ffc2bdaa8e3b5de45`.
- [GitHub Release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.2.0)
  is latest, published/non-prerelease, with the exact committed
  [versioned notes](../releases/v0.2.0.md) and no attached distribution assets.
- [Full published-documentation run 43143961474](https://github.com/rustcommons/rust-mirror/actions/runs/43143961474)
  and [release-event check 43143958798](https://github.com/rustcommons/rust-mirror/actions/runs/43143958798)
  passed; local `check-release-docs.py --published` also passed.
- [crates.io](https://crates.io/crates/rust-mirror/0.2.0) serves the concise packaged
  README. Versioned [docs.rs](https://docs.rs/rust-mirror/0.2.0/rust_mirror/),
  [events module](https://docs.rs/rust-mirror/0.2.0/rust_mirror/events/) and
  [ErrorKind](https://docs.rs/rust-mirror/0.2.0/rust_mirror/enum.ErrorKind.html)
  pages returned HTTP 200 with generated API content.

The GUI maintainer was notified immediately after the coordinator's upload report;
registry installation is now independently verified. No second upload, tag move,
standalone binary publication, version change or production operation was needed.
G5 exact config-aware resume inspection, private-workspace deletion, in-generation
compaction and Nexus pruning remain planned/deferred as documented. Cooperative
cancellation, blocked I/O, partial effects and filesystem trust limits still apply.

Machine-readable inspection records: [archive/payload](0.2.0-payload-verification.json),
[registry-installed identity](0.2.0-registry-identity.json),
[tag/release](0.2.0-github-release-verification.json) and
[published pages](0.2.0-published-pages.json). These contain public identities and
bounded results, not credentials or fixture private keys.
