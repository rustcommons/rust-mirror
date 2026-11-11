# Rustup lifecycle development, 2026-11-11

Owner scope: complete selected Rustup workflows and actual Nexus qualification for
0.1.0, with scheduling excluded. This is an evolving development record, not a
release approval. See the [gates](../RELEASE-0.1.0-PLAN.md).

## Research and measured upstream behavior

Rustup 1.28.2 [self-update source](https://github.com/rust-lang/rustup/blob/1.28.2/src/cli/self_update.rs)
reads `release-stable.toml` then `archive/VERSION/HOST/rustup-init[.exe]` below
RUSTUP_UPDATE_ROOT. The original shell installer also uses `dist/HOST` when no
version override is provided. The mirror must cover both layouts. Panamax's
[Rustup implementation](https://github.com/panamax-rs/panamax/blob/master/src/rustup.rs)
was inspected as a reference; no source was imported.

The actual upstream release catalog reported schema 1 / Rustup 1.29.3. The
[13-case live sweep](rustup-endpoint-cases-20261111.json) obtained valid checksum
sidecars for Linux GNU/musl, Windows MSVC/GNU, macOS and additional advertised
architectures. All returned HTTP 200 with parseable SHA-256. These are metadata
checks, not foreign-platform installations. Pinned archive checksums avoid assuming
that a host's latest public bootstrap matches the mirror's selected Rustup version.

## Implementation and pending evidence

The finishing command adds a generated, explicitly pinned Rustup catalog and
original installer/script bytes to an immutable verified generation. It retains
old archives and supports explicit original dated channel promotion. Four focused
Rustup tests and CLI refusal coverage are included. Source changes do not imply
native qualification. Record actual Windows/macOS/Linux and Nexus results here
before closing release gates. No new dependencies or version/lock change.
