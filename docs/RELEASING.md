# Release policy and checklist

The first published crate is `rust-mirror` 0.1.0. No placeholder crate is published
for name reservation; `rust-mirror-gui` remains future work. Publication requires
the owner's separate go-ahead. This policy does not itself authorize publication.

## Versioning and scope

Use a minor release for new CLI/report/refusal contracts while pre-1.0; use a patch
for compatible fixes and documentation-only package corrections. Keep saved schema
compatibility explicit and migrations documented. The current release scope is
selected modern Rustup distributions, offline clients and verified Nexus Raw
publication. Scheduling, Cargo registry mirroring, GUI and pruning were excluded
from 0.1.x. Local whole-generation retention/pruning shipped in 0.2.0;
Nexus pruning and in-generation compaction remain gaps. The owner authorized 0.2.0 publication on 2026-11-11 after completion, merge and
qualification, including protocol/controls/typed errors and publish preflight.
This authorization does not waive any permanent release gate.

The owner separately authorized 0.3.0 publication on 2026-11-12. The coordinator
published corporate networking from the owner's machine after normal merge;
[postpublication verification](evals/2026-11-12-publication-0.3.0.md) records the
registry, tag/release, installed proof and docs.rs. Later versions still require
separate owner authorization and all applicable gates.

## Before merge and publication

1. Update the changelog, versioned notes, user guide, complete CLI reference and
   stable ROM-REQ trace together. Audit every command/option against the source,
   including defaults, outputs, local/remote effects and recovery.
   Keep historical failures and bounds; metadata checks are not native evidence.
2. Run Rust 1.90 fmt, tests, Clippy and rustdoc with warnings denied, Python tests,
   link/render checks and release-documentation CI. Deny rustdoc warnings and
   compile packaged walkthrough examples. Inspect the final diff. For docs-only
   patches, prove runtime tokens/dependency identities unchanged and carry native/
   Nexus evidence in its original scope instead of repeating unrelated matrices.
3. Run the [Rustup lifecycle workflow](../.github/workflows/rustup-lifecycle.yml)
   for Linux/x86-64, Windows/x86-64, macOS/Intel and Apple Silicon. Require fresh
   bootstrap, old/new stable and pinned history, numeric aliases, minimal/default profiles, offline documentation, components/targets,
   offline build, self-update/repeat and missing/corrupt controls. Linux additionally
   checks beta/nightly. Preserve isolation and temporary-CA cleanup evidence,
   including explicitly recorded macOS hosted-VM teardown if its removal API blocks.
   Require destroyed fixture private keys; never accept this route on a self-hosted VM.
4. On actual disposable Nexus, require schema 2/3, native CLI publication, private
   CA/hostname/auth/no-redeploy controls and same-client updates through a fixed
   Raw group URL. Never accept EULAs or use production credentials for qualification.
5. Audit `cargo package --locked`: exact clean VCS, include list, source/manifest/
   lock/license bytes and no private evidence/keys/archives. Unpack and install
   with `cargo install --path UNPACKED --locked --root FRESH_ROOT` and a fresh
   Cargo home. Check `--version`, CLI/refusals and real native publication/lifecycle
   using the installed executable. A source-tree pass is not a package-install pass.
6. Freeze the reviewed candidate, merge normally, verify tree equality and repackage
   at the exact merge. Compare payloads; only VCS metadata may differ. Record the
   archive hash. Packaged README/changelog changes require a refreshed archive and
   candidate identity even when native evidence carries by unchanged code. Require
   owner publication authorization before `cargo publish`.

## After publication

- Verify the official crates.io archive against the API/index checksum, non-yanked
  status, exact release VCS and qualified payload; verify docs.rs build and links.
- Perform a fresh actual `cargo install rust-mirror --version VERSION --locked`
  and installed-command proof. Do not claim registry installation before upload.
- Create the annotated version tag at the exact release commit. Publish the exact
  committed [versioned notes](releases/v0.3.0.md) as the GitHub Release, latest and
  without attached standalone binaries unless separately authorized.
- Verify live tag/body, full `--published` documentation CI, registry/docs agreement
  and clean remote-aligned main. Record evidence in excluded documentation.
- Never move a release tag, overwrite a published crate or rewrite failed history.
  A discovered published-doc defect requires a new version if its package must change.

## Update triggers and limitations

Rerun affected permanent gates when Rustup endpoint/schema/client behavior changes,
new hosts/components/formats are advertised, Rust toolchain metadata changes,
Nexus/proxy/TLS/auth policies change, or download/publication/selection code changes.
An unchanged excluded-doc correction needs only its narrow checks. A new untested
host or server stays unqualified until native evidence is recorded.

The mirror retains upstream signatures but does not independently verify OpenPGP.
TLS/upstream checksums and trusted administrative transfers are the stated trust
model. No evidence certifies arbitrary offline Cargo dependencies, OS SDKs, ancient
pre-v2 Rustup releases, hostile filesystems, production configuration or globally
atomic in-flight group switching. Use the [current gates](RELEASE-0.1.0-PLAN.md) and
[dated evidence](evals/2026-11-11-rustup-lifecycle.md), not a blanket coverage claim.
