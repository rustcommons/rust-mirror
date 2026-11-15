# Release policy and checklist

The first published crate is `rust-mirror` 0.1.0. No placeholder crate is published
for name reservation. GUI work is governed separately. Publication requires
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

The owner authorized 0.4.0 publication on 2026-11-13: accepted crate MVP plus main.
The coordinator normal-merged the reviewed release PR and published from the owner's
Windows machine; [postpublication verification](evals/2026-11-13-publication-0.4.0.md)
records the exact registry payload, documentation and installed proofs. Carry the
[Phase 1 evidence](evals/2026-11-13-crates-mvp.md) only where code is unchanged.
Crate qualification additionally requires the dated top-100 corpus/byte comparison,
isolated HTTPS Cargo clients, kill/cancel/resume and actual schema-4 Nexus proof.
Keep bounded client builds, Linux-only Nexus client evidence and historical-cohort
refusal explicit. See [0.4.0 notes](releases/v0.4.0.md). This is not GUI Phase 2 approval.

The owner authorized 0.5.0 publication once complete and qualified on 2026-11-13.
Require the [lock-import gates](LOCK-IMPORT.md#qualification-before-implementation-acceptance)
in addition to existing gates, plus legacy schema/digest compatibility and repeated
pruning. The coordinator reviews/merges and publishes from the owner's machine;
[0.5.0 notes](releases/v0.5.0.md) identify new behavior and limits. The coordinator
published after normal merge; [postpublication verification](evals/2026-11-13-publication-0.5.0.md)
records registry payload, docs.rs, fresh installation, tag/release and workflow evidence.

The owner authorized qualified 0.5.1 publication for historical build-metadata
index collisions, resumed on 2026-11-15. The coordinator reviews/merges and publishes
from the owner's machine after green checks. Require Cargo 1.90 full/filtered-index
conformance for real ndk-sys, synthetic selected/unselected conflicts and windowing
regressions; preserve selected/retained checksum refusal and exact index bytes.
See [0.5.1 notes](releases/v0.5.1.md) and [bounded evidence](evals/2026-11-15-index-collisions.md).
No runtime dependency or resource-default changes are part of this patch.
The coordinator published from the owner's Windows machine after normal merge;
[postpublication verification](evals/2026-11-15-publication-0.5.1.md) records the
registry payload, docs.rs, fresh installation, tag/release and workflow.

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
  committed [versioned notes](releases/v0.5.1.md) as the GitHub Release, latest and
  with binary archives/checksums attached only through the release workflow under
  the owner's 2026-11-15 authorization. Do not change historical note bodies.
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

## Prebuilt GitHub Release binaries

The owner authorized binary attachments on 2026-11-15, superseding the earlier
notes-only policy for future distribution. This does not authorize a new crates.io
publication, a version bump, OS signing or rewriting previous release outcomes.
Backfill only current v0.5.1 after coordinator review/merge of this workflow.

[Release binaries](../.github/workflows/release-binaries.yml) runs on v* tag pushes
and workflow_dispatch with a tag and dry_run (default true). Each build checks out
the resolved immutable tag commit, uses Rust 1.90.0 and cargo build --release
--locked --target TARGET. A changed MSRV needs an explicit workflow/helper review.
Build targets are x86_64-pc-windows-msvc on windows-latest,
x86_64-unknown-linux-gnu on ubuntu-22.04, aarch64-apple-darwin on macos-15 and
x86_64-apple-darwin on macos-15-intel. Ubuntu 22.04 is the oldest practical hosted
Ubuntu runner; record its glibc 2.35, actual maximum required GLIBC symbol and
linked libraries. Windows release builds alone set RUSTFLAGS to
-C target-feature=+crt-static; do not change repository-wide configuration, normal
CI or development builds. Audit native DLL imports with vswhere + dumpbin
/dependents and the bounded PE parser: fail if no imports are parsed or if any
match ^(vcruntime|msvcp|ucrtbase|api-ms-win-crt-) case-insensitively. Record actual
system DLL imports; no Visual C++ Redistributable is required by these archives.
Fail if the baseline rises or required symbols exceed 2.35.
This bounds runner compatibility, not all distributions or older OS versions.

Each rust-mirror-VERSION-TARGET.zip (Windows) or .tar.gz (Unix) contains one
same-named folder with the executable, original tagged LICENSE and README.txt.
The README records version, exact source commit, usage, verification, unsigned OS
prompts and Linux ABI evidence. Smoke-test --version and protocol-1 --version
before packing and after extraction on each native runner. Collect only the four
expected archives, verify source/target/license/digests and produce sorted
SHA256SUMS with lowercase SHA-256, two spaces and file name. Preserve build reports
as workflow artifacts; record exact run IDs and measured ABI in dated evidence.

Prepare/build/verify jobs are read-only. Only the publish job has contents: write,
id-token: write and attestations: write. It uses actions/attest-build-provenance to
attest all four archives and SHA256SUMS with Sigstore, then gh attestation verify
with the repository/workflow identity before uploading. Dispatch provenance names
the main workflow revision; the attested archive README binds the separate tagged
source commit. Do not claim the workflow revision is the backfilled source revision.
Tag-triggered builds use the same source/workflow revision.

The workflow never creates a GitHub Release: postpublication still creates exact
committed notes. It waits up to ten minutes for that existing release, then refuses
mismatched tag/commit/body or drafts. The publish job rechecks immediately before
writes and refuses **any existing assets**, including an earlier partial upload.
GitHub duplicate-name refusal is retained; no clobber, PATCH, DELETE or automatic
repair is permitted. Concurrent publish jobs for the same tag are serialized.
An interrupted upload may leave a successful prefix: preserve it and ask the owner
for a recovery decision; never delete/rebuild existing published files.

A PR-triggered read-only v0.5.1 build is the bootstrap test when GitHub cannot
register workflow_dispatch until the file exists on main. Dispatch dry runs build,
smoke-test, collect and check release/tag/body but never attest/upload; they allow
existing assets because they cannot change them. Live dispatch must run the reviewed
workflow on main with dry_run=false. After merge, run a dispatch dry run first,
then dispatch v0.5.1 with dry_run=false. Do not create a public throwaway release.
After uploading, download all five live assets, verify SHA256SUMS and attestations,
inspect archive layout/README/source, recheck release body/tag and record evidence.

See [install verification and unsigned prompts](INSTALL.md). GitHub provenance is
separate from Windows Authenticode/Apple Developer ID or notarization; no paid OS
signing is configured. Keep historical no-assets records dated and intact. New
assets do not change immutable crates.io payloads or the release's source tag.
