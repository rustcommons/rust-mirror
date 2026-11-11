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

## Final prepublication qualification

The implementation merged through [PR12](https://github.com/rustcommons/rust-mirror/pull/12)
as `7c020b58b9893d41775157710d98a895ede66dd8`, tree-identical to qualified
`4738aef30187431886c67aa931af0eeaef681fc4`. All jobs in
[run 42961977999](https://github.com/rustcommons/rust-mirror/actions/runs/42961977999)
passed, as did portable Linux/Windows CI 42961977995 and release-documentation CI.
All four artifact ZIPs were downloaded and checked against their GitHub SHA-256
metadata; recorded command statuses and default-profile coverage were inspected.

| Native host | Passing client commands | Additional scope |
| --- | --- | --- |
| x86_64-unknown-linux-gnu | 44 | Stable, beta/nightly, minimal/default, numeric aliases, offline docs |
| x86_64-pc-windows-msvc | 38 | Minimal/default, numeric aliases, offline docs |
| aarch64-apple-darwin | 38 | Minimal/default, numeric aliases, offline docs |
| x86_64-apple-darwin | 38 | Minimal/default, numeric aliases, offline docs |

Each host checks genuine Rust 1.89.0 to 1.90.0 stable-client updates, retained pinned
history, component/target changes, offline build/run/Wasm metadata, Rustup 1.28.2 to
1.29.3 self-update/repeat and missing/corrupt download controls. Expected refusals
are passing negative controls, not successful installs. Linux additionally installs
and repeats the exact beta/nightly versions recorded in the machine-readable record.
The native publisher's private-CA/hostname refusals, successful verified upload and
repeat-prefix refusal are included on all four hosts.

Actual isolated Nexus OSS 3.76.1-01 passed 20 collection security/protocol cases,
44 client commands through the same Raw group URL, and a separate 18-case selected
snapshot proof. Both owned containers/volumes were removed. The official image is
pinned at `sha256:390b8ec7e712c680ed8b449b766695a7c5ebbf41e489317618e63ca199f2cc09`.
This qualifies that legacy compatibility fixture, not a production recommendation
or current Community Edition/license policy.

A clean 18-file crate of `a82d390` passed fresh unpacked `--locked` installation,
nine CLI checks, four host metadata listings, actual Nexus (20 cases / 36 client
commands) and a separate default-profile 38-command Linux lifecycle. Cargo metadata
resolved 101 non-root packages from crates.io; there were no path/git replacements.
The `4738aef` archive differs only in VCS metadata and the changelog; compiled
source, examples, manifest, lock, README and license are identical. The final
release-record update additionally makes the packaged changelog's note link
absolute; it changes no compiled input. Final clean-merge packaging is rechecked
before requesting publication. These are unpacked installs, not a nonexistent
registry installation of an unpublished version.

The [machine-readable record](rustup-0.1.0-qualification.json) contains refs,
artifact hashes, command outcomes, package identities and limits. Local raw evidence
is preserved in this thread's `rust-mirror-rustup-complete-20261111` directory.
All 21 original prototype hashes remain unchanged.

### Preserved setup failures and corrected coverage

- Initial macOS runner/certificate and Windows firewall/Cert-provider failures above
  remain historical failures, not passes.
- Run 42958296997 exposed blocking headless certificate cleanup/import commands.
  Current-user Windows trust operations also blocked; machine-store setup/removal
  subsequently passed. Native trust setup refuses workstations/self-hosted runners.
- macOS client commands passed before its removal API timed out. The permanent
  harness now records that timeout honestly, destroys fixture private keys and
  relies on fresh GitHub-hosted VM teardown for remaining test trust state. It
  never claims explicit removal succeeded; no production/user trust store is used.
- Run 42960893441 was green, but artifact inspection found Intel macOS's default
  profile skipped: upstream reuses an ARM-named documentation archive. The harness
  now uses the validated selection and requires actual default-profile/doc commands.
  Run 42961977999 confirms those commands ran successfully on every native host.

Implementation/native/server/package gates are complete. Registry publication,
official archive/install, docs.rs, tag and GitHub Release are still pending the
owner's separate authorization; no completion of those future steps is claimed.

## Previous frozen clean-source package

Release candidate: `0b38e4595a09e5e2a5b781c39b509fb60eff3d9c` (PR13 merge).
This is now a historical qualified package: the later user-manual/README/changelog
update supersedes it for publication. Preserve its evidence; do not upload that
older archive as the documentation-complete candidate.
Final `cargo package --locked` verification and a fresh-Cargo-home unpacked locked
install both passed with Cargo/Rust 1.90.0. The 18-file archive is 48,883 bytes:

```text
360885af0d2b89a1ec0ae7146ea3ea30c1e901943cc5b8d51999477efb8fdd9f
```

Clean VCS names that exact commit. Every included source/original-manifest/lock/
README/example/license byte matches it. Compared with installed/qualified `a82d390`,
only VCS metadata and the changelog differ; normalized manifests are byte-identical.
The final installed CLI passed version/help/init/existing-output/unknown-option
checks and the namespace-isolated private-CA publisher proof, including unknown-CA,
wrong-hostname and repeat-prefix refusals. All 24 exported evidence-file hashes
were verified. Broad native/Nexus evidence carries by unchanged compiled inputs;
it was not misleadingly described as a new full runtime rerun.

This later bookkeeping changes excluded docs only. Keep publication/tag source at
the frozen candidate above; it is still unpublished. Official registry installation,
docs.rs, tag and Release verification must follow the owner's publication go-ahead.

## Documentation-complete candidate refresh

The previous frozen-package record above is historical. Full operating/reference
documentation merged in PR15. The current frozen candidate is
`93f38921000ba4e8cc1d55006afd326c077d8f7e`; its clean Rust 1.90 package verification passed.
Only README/changelog/VCS differ from the earlier archive; compiled inputs remain
unchanged. See the [documentation/package record](2026-11-11-user-documentation.md#documentation-complete-clean-package)
for the new hash, exact comparisons and CI observation. These are documentation
and package checks, not a new installation/native/Nexus run. Publication remains
on hold for owner authorization.

## Published-state follow-up

The owner authorized publication after the full documentation was completed.
0.1.0 is now on crates.io from the documentation-complete `93f38921` source.
The [publication record](2026-11-11-publication.md) separately records official
archive identity, docs.rs, actual registry installation and installed-command
checks. Earlier prepublication holds/failures above are historical, not current
publication status; their original evidence and scope are preserved.
