# rust-mirror 0.1.0 publication, 2026-11-11

The owner explicitly authorized publication after full documentation completion.
Source/tag: `93f38921000ba4e8cc1d55006afd326c077d8f7e` / `v0.1.0`.
The later records change excluded documentation only. No version was republished
and no source tag was moved.

## Official archive and documentation

- [crates.io 0.1.0](https://crates.io/crates/rust-mirror/0.1.0): non-yanked,
  minimum Rust 1.90, 18 files, 49,142 bytes.
- SHA-256: `b6e01cc29cefc18121c54c23230238a076c9935b8823523073021f8cdba7a69c`.
- Official archive, crates.io API and sparse-index checksums match. The downloaded
  bytes are identical to the qualified documentation-complete archive; clean VCS
  names the exact source above.
- [docs.rs build/API](https://docs.rs/rust-mirror/0.1.0/rust_mirror/) succeeded
  (`doc_status: true`). The [published README](https://docs.rs/crate/rust-mirror/0.1.0)
  contains working user-guide/reference links, each checked at HTTP 200.
- Annotated tag `v0.1.0` peels to the release source, with message
  `rust-mirror 0.1.0`.

## Actual registry installation and installed commands

`cargo install rust-mirror --version 0.1.0 --locked --root FRESH_ROOT -j 2`
ran with Rust/Cargo 1.90, a fresh Cargo home, isolated root and no project/path
patch overrides. The log records download of rust-mirror 0.1.0 from crates.io;
this is an actual registry installation, not an unpacked candidate install.

Installed binary SHA-256:
`56e348ffbc4d0157b02720dad4d7041ae15e56eeb0c22960485b8864dd255f9c`.
Version, help and initial config creation returned 0; existing config and unknown
publisher-option refusals returned 1. The installed native publisher then passed
an isolated loopback-only network-namespace HTTPS proof: explicit private CA,
scoped Basic credentials, nine published/verified files, inventory last, and
unknown-CA/wrong-hostname/repeated-prefix refusals. Both temporary private keys
were destroyed, and the owned server stopped normally. This is a protocol fixture,
not a new actual Nexus test or native client matrix.

The original [four-host/Nexus evidence](2026-11-11-rustup-lifecycle.md) carries only
for unchanged compiled source/dependency inputs. No arbitrary-host, old-OS,
production Nexus/CE licensing, SDK or offline Cargo-registry guarantee is added.
Original signature files are retained without independent OpenPGP verification.

## Release completion and retained evidence

The [versioned notes](../releases/v0.1.0.md) are the canonical GitHub Release body.
Completion requires an exact live body/source tag, latest status, zero assets,
and green full published-documentation CI under the [release policy](../RELEASING.md).
Those live results are retained with this release's workflow/PR completion record;
the archive and tag remain immutable.

Detailed logs, API/index snapshots, official archive, command outputs, namespace
proof and per-file hashes are in this thread's `rust-mirror-publication-20261111`
storage directory. Initial evidence manifest: 56 files, SHA-256
`7cc399a1b40494bdad91f6c6bea6c50d498f2c9b91d1f83332e5e02bacb5f3b6`. No credential values or fixture private keys are included.
