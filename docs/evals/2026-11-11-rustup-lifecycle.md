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


The [36-case artifact size sweep](rustup-artifact-size-cases-20261111.json) sampled
actual gzip artifact HEAD responses for Rust 1.90.0, current stable/nightly, Linux,
Windows and macOS, including rustc, documentation, rustc-dev and LLVM tools. All
36 returned known sizes below the 512 MiB per-file limit (largest 246,839,325 bytes).
This measures those exact URLs; future upstream versions/formats still need checks.

## Intermediate results (not final release approval)

- Native WSL/GNU Linux: 31 lifecycle cases passed inside a loopback-only namespace,
  including genuine bootstrap, old/new stable, components/targets, offline builds,
  self-update and negative controls.
- Actual isolated Nexus OSS 3.76.1-01: schema-3 old/new publication/lifecycle passed,
  then fixed Raw-group URL activation and the same existing-client lifecycle passed.
- A separate schema-2 publication/bootstrap/install/build proof passed on that image.
  Containers/volumes were owned, network-disabled and removed afterward.
- Hosted release-built Linux qualification at run 42950688394 passed native
  stable/beta/nightly and the paired actual Nexus tests. Its application source
  predates the later numeric alias addition; it is bounded intermediate evidence.
- Windows and macOS final native acceptance remains pending. Initial failures were
  test-setup issues: retired macOS runner label, a CA used as the server certificate,
  unsupported Windows firewall address forms, and a fresh PowerShell process lacking
  the Cert provider. These are retained as failures, not counted as passes. The
  corrected CA/leaf fixture has a permanent trusted/untrusted handshake test.

The finishing command now supplies explicit full/minor numeric policy aliases and
records the selected highest stable patch. Final native acceptance must check
those new client lookups too. No release or crates.io publication is authorized yet.
