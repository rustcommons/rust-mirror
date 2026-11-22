# Release policy and checklist

Never use `[skip ci]` or `[ci skip]` in release, documentation, commit or merge
messages. Required CI and GitHub-managed CodeQL must execute normally. Release
workflow installs follow [the pinned-install rules](SECURITY-BASELINE.md#pinned-installs-and-sast).


The first published crate is `rust-mirror` 0.1.0. No placeholder crate is published
for name reservation. GUI work is governed separately. Publication requires
the owner's separate go-ahead. This policy does not itself authorize publication.

## Current publishing route

Owner decision, 2026-11-16: the PC crates.io token was revoked. **Trusted Publishing
is the only authorized crates.io publish route.** Use
[trusted-publishing.yml](../.github/workflows/trusted-publishing.yml), the crates-io
environment's owner approval and its short-lived OIDC token. Do not upload crates from a
workstation, create/restore a long-lived Cargo token or use a manual fallback.
Each version still needs owner authorization and all qualification/verification gates.

### Standard release pipeline

Owner approval, 2026-11-21: [trusted-publishing.yml](../.github/workflows/trusted-publishing.yml)
follows the Rust Commons [standard pipeline](https://github.com/rustcommons/.github/blob/main/docs/TRUSTED-PUBLISHING.md).
Phase 2 (owner-approved the same day) makes it the only release workflow: it builds,
signs and publishes the crate and all binaries. The flow is:

1. **Release notes final before tagging.** The release PR moves the `Unreleased`
   CHANGELOG entries under a dated `## [X.Y.Z] - YYYY-MM-DD` heading and commits
   `docs/releases/vX.Y.Z.md` in final form. The GitHub Release body is that file.
   The PR's dry run (gate plus the full build, publish=false) must be green. Fork PRs
   skip the dry-run build (the reusable call requests id-token: write, which forks
   cannot receive), so a release PR must come from a branch of this repository.
2. **Tag.** After the coordinator's normal merge and green CI on that main commit,
   push an annotated `vX.Y.Z` tag on the merge.
3. **Release gate.** Refuses the run unless the tag is annotated and names the
   version, the commit is reachable from main with successful CI, the dated
   CHANGELOG entry, notes and release-documentation check pass, the tag record
   equals the commit, and the workspace is exactly the `rust-mirror` crate.
4. **Build** ([isolated reusable builder/signer](../.github/workflows/build-binaries.yml),
   which runs the [read-only core](../.github/workflows/build-binaries-core.yml)).
   The core's `crate-sbom` job runs every third-party tool (cargo-cyclonedx, cargo
   package/install); the `verify` job that downloads the binaries and writes
   SHA256SUMS runs only this repository's stdlib Python, and the signer re-checks
   every hash.
   Five native builds with every qualification gate (below), the SBOM from the clean
   frozen source with pinned cargo-cyclonedx 0.5.9, `cargo package --locked` with a
   package audit (regular members equal to the commit, clean VCS record, no fuzz/CI
   content) and a packaged install (version, protocol 1, typed refusal), then one
   SHA256SUMS over the .crate, the five archives and the SBOM.
5. **Build / Checksums and attestations** (nested in the same reusable workflow; there
   is no top-level `attest` job here). Only on a v* tag push it re-verifies the tag
   record, source and workflow commit and the exact file set, signs SLSA provenance
   for all eight files, binds the SBOM to each archive and the crate, and stages one
   bundle `rust-mirror-X.Y.Z.intoto.jsonl`. Because the signer also ran the build,
   its signature proves the qualified build produced the bytes. PR and dispatch runs
   never sign. All of this finishes before approval; `publish` needs `gate` and `build`.
6. **Owner approval, then publish.** The job rechecks any existing publication against
   the exact tag payload and skips a version crates.io already has. **Nothing compiles
   in the publish job**: it holds `id-token: write` and the crates-io environment, so
   no crate or dependency code (build scripts, proc macros) may run there. It runs
   `cargo package --locked --no-verify` (packaging only; the build job already built
   the packaged source) and `cmp`s the result with the attested `.crate`. Only then
   does crates-io-auth-action mint the short-lived token, and only the final step,
   `cargo publish --locked --no-verify`, receives it.
7. **Automatic GitHub release.** `github-release` re-stages the bundle (subjects equal
   the eight signed files), requires every attached DSSE envelope to equal GitHub's
   stored attestation and the SBOM attestation of every archive and the crate to
   carry this exact SBOM. It then creates `vX.Y.Z` as a **draft** from the committed
   notes, uploads all nine assets, re-reads and checks them, and only then publishes
   the release (`draft: false`, latest). **The maintainer no longer creates the
   GitHub Release by hand.**

   *Recovery:* if the job is killed, re-run it. The tags API returns 404 for drafts,
   so the job finds the draft through the releases list. Before adding anything to an
   existing release (draft or published) it refuses unless the tag, title, body (equal
   to the committed notes) and draft/prerelease state are exactly as expected and every
   existing asset is one of the nine with identical bytes. It uploads only missing
   assets, checks all nine, then publishes. A published release is never modified: a
   complete one makes the re-run a no-op, an incomplete one is refused for a decision.
   Two releases for one tag are refused. Never delete or replace an asset to pass.

   *Manual recovery, two cases.* (1) A killed upload can leave a half-uploaded asset
   on the draft; the job then refuses it as differing. Delete only that asset from the
   draft by hand (the release is still a draft, so nothing public changes), then re-run
   only the failed `github-release` job. (2) Do not use **Re-run all jobs** after a
   partial draft: that rebuilds and re-signs, and the archives are not bit-identical
   across builds, so the new files would differ from those already on the draft and be
   refused. Re-run only the failed jobs, which reuse this run's built and signed files.
   If a full re-run is unavoidable, first delete the draft's assets by hand, then re-run.
   Never delete or replace an asset of a published release.
8. **Verify publication.** On Windows, musl and GNU runners it downloads every asset,
   checks digests, SHA256SUMS, every attestation (signer build-binaries.yml, source
   and signer digest = tag commit, no self-hosted runners), the bundle against the
   store and the SBOM binding, then executes the downloaded binary. The GNU leg also
   checks the live body, the registry checksum and exact tag payload, the crates.io
   checksum against the attested `.crate`, docs.rs (pending is reported as pending)
   and a clean `cargo install --locked` from crates.io.

**Asset profile from the next release (after 0.6.0), nine files:**
`rust-mirror-X.Y.Z.crate`; `rust-mirror-X.Y.Z-aarch64-apple-darwin.tar.gz`,
`-x86_64-apple-darwin.tar.gz`, `-x86_64-pc-windows-msvc.zip`,
`-x86_64-unknown-linux-gnu.tar.gz`, `-x86_64-unknown-linux-musl.tar.gz`;
`rust-mirror-X.Y.Z.cdx.json`; `SHA256SUMS` (seven sorted entries: the .crate, five
archives, SBOM); `rust-mirror-X.Y.Z.intoto.jsonl` (signs those seven and SHA256SUMS).
Releases v0.5.1 to v0.6.0 keep their original profiles; the verifiers check each
exactly as before. The interim `SHA256SUMS-crates`/`.crates.*` names from the first
adoption were never released.

[release-binaries.yml](https://github.com/rustcommons/rust-mirror/blob/v0.6.0/.github/workflows/release-binaries.yml)
is retired: no tag trigger remains. Every release up to 0.6.0 is complete (0.5.3 is
crate-only by owner decision), so no exact-tag recovery is pending. A recovery
dispatch at an old tag would run that tag's own copy, but GitHub only dispatches a
workflow that also exists on the default branch, so reviving it needs a reviewed
PR that restores the file.

The owner-machine publications and publish-then-tag sequences below are historical
release records, not currently available fallbacks. Preserve their source identities,
checksums, tags and outcomes. Current releases tag the reviewed merge before
Trusted Publishing; see [the operational guide](SECURITY-BASELINE.md#trusted-publishing-only).

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

The owner authorized 0.5.2 publication on 2026-11-15 for five prebuilt downloads,
including the owner-selected cargo-libc 0.7.0 glibc 2.17 GNU variant. Mirroring
runtime source and dependency identities are unchanged from 0.5.1; carry the named
0.5.1 native/client/Nexus evidence only within its original bounds. Require fresh
version/package/doc checks and exact-candidate five-platform binary qualification,
including all eight GNU userland gates. The coordinator merges and publishes from
the owner's machine; tag/release/assets and registry/download verification follow
publication confirmation. See [0.5.2 notes](releases/v0.5.2.md). The coordinator
published after normal merge; [postpublication verification](evals/2026-11-15-publication-0.5.2.md)
records registry/docs.rs, all six public assets/attestations, native Windows/GNU/musl
downloads and registry installation.

## 0.5.3 Trusted Publishing release

Owner authorization, 2026-11-15: release the merged RustSec dependency fixes plus
existing main as 0.5.3; no further application code. See [security notes](releases/v0.5.3.md)
for all upstream IDs and bounded exposure. Preparation allowed a draft repository
advisory; the coordinator retained publication authority until release verification.

The coordinator merges the green release PR. Unlike the manual publish-then-tag
route below, create the annotated v0.5.3 tag at the exact merge **before registry
publication**. Verify merge/candidate tree equality first. The canonical
[trusted-publishing.yml](../.github/workflows/trusted-publishing.yml) qualifies the
package and waits for the owner's crates-io environment approval, then uses its
short-lived OIDC token. Never supply a long-lived Cargo token or bypass the review.
After confirmed registry publication, create the GitHub Release from the exact
committed notes; let release-binaries attach the five archives, SBOM and SHA256SUMS.
If owner approval outlasts the binary workflow's bounded release wait, retry the
non-dry workflow at the exact immutable tag after the release exists, only with no
existing assets. Never delete/replace a successful upload prefix.

Verify the registry checksum/payload against the merge, docs.rs, a fresh registry
install, all seven files, reusable signer/source commit digests, archive SBOM
bindings and actual downloaded Windows/GNU/musl executables. Keep candidate and
publication evidence distinct; record the first real Trusted Publishing outcome
without assuming success. The 0.5.3 authorization selected Trusted Publishing while
the earlier manual process still existed. The owner revoked the PC token on
2026-11-16; the current route above supersedes that historical fallback. Existing
release artifacts and outcomes remain unchanged.

The [0.5.3 publication checkpoint](evals/2026-11-16-publication-0.5.3.md)
records successful Trusted Publishing, registry/docs.rs/install verification and
both failed binary attempts: initial collector timeout, then a missing SBOM serial
rejected by the pinned attestation action. No binary assets were uploaded. Keep
this partial outcome separate from the permanent checklist. On 2026-11-16 the
owner rejected any different-builder recovery exception for 0.5.3 and selected
0.5.4 instead. The 0.5.3 release remains crate-only with its exact historical body.

## 0.5.4 download security release

Owner decision and authorization, 2026-11-16: do not recover/backfill 0.5.3 under a
new builder or loosen source/signer identity. Its crate is published with patched
dependencies; no prebuilt assets were uploaded. Preserve the protected tag, exact
historical GitHub Release body and both failed attempts. The owner selected a new
0.5.4 release containing the merged SBOM serial fix and the same application code/
locked dependency identities. See [0.5.4 notes](releases/v0.5.4.md).

Qualify the exact clean PR head, package and five-target/seven-file dry run with
all eight GNU gates. The dry run checks final SBOM shape, serial, graph/source/lock,
archive collection and checksums on the shared build path; OIDC signatures and
archive/SBOM bindings are verified only after an authorized merged tag. The
coordinator reviews/merges, then create v0.5.4 on that normal merge. Trusted
Publishing waits for owner environment approval. Confirm publication, create the
exact notes, let the tag workflow attach assets, and perform all registry/docs.rs/
installation/provenance/SBOM/public Windows/GNU/musl gates before recording completion.
A collector timeout may use the approved exact-tag dispatch with an empty release,
never a new source/signer revision. The advisory identifies <=0.5.2 affected,
patched crate 0.5.3 and patched downloads 0.5.4; publication was coordinator-owned.

The [completed 0.5.4 record](evals/2026-11-16-publication-0.5.4.md) identifies the
actual registry/tag, all seven assets/provenances, all five SBOM bindings, docs.rs,
fresh installs and public Windows/GNU/musl execution. Preserve its initial Windows
reader-locale failure and corrected UTF-8 proof. Verification tools must decode
GitHub CLI JSON and proof files explicitly as UTF-8 and compare the exact signed
SBOM; never normalize away a mismatch. Reader fixes do not authorize producer/tag/
asset changes or a source/signer exception. The coordinator published
[GHSA-46c8-w6pp-97pw](https://github.com/rustcommons/rust-mirror/security/advisories/GHSA-46c8-w6pp-97pw) at 2026-11-16T05:35:02Z after verification.
Keep earlier draft observations dated and older immutable profiles unchanged.

## 0.6.0 dependency release

Owner authorization, 2026-11-21, released as 0.6.0 by coordinator decision under
the owner's 2026-11-15 minor-release allowance: 0.6.0 ships existing main with the merged
dependency batch (sha2 0.11, toml 1.2, webpki-roots 1.0 direct, serde 1.0.229,
tempfile 3.27). Application source is byte-identical to 0.5.4. See
[0.6.0 notes](releases/v0.6.0.md). The public `Error::report` field exposes
`toml::Value`, so toml 0.8→1 is a SemVer break for library users and the release is
minor (0.6.0), not a 0.5.x patch. The upgrade notes state the break.

This is the first release with the attached provenance bundle (eight assets):
five archives, the SBOM, SHA256SUMS and rust-mirror-0.6.0.intoto.jsonl. Qualify the
exact clean PR head, the package and the candidate dry run with all eight GNU gates
and the `provenance-dry-run` job. After the coordinator's normal merge, tag v0.6.0
at that merge; Trusted Publishing waits for owner approval. Then create the exact
notes and let the tag workflow upload. Verify the registry, docs.rs, a fresh install,
all eight assets, the bundle's match with GitHub's attestation store, the SBOM
bindings and public Windows/GNU/musl execution before recording completion. A
collector timeout may use only the exact-tag dispatch with an empty release.

The [completed 0.6.0 record](evals/2026-11-21-publication-0.6.0.md) documents that
path in use: tag run 44906731143 timed out before owner approval of publishing and
uploaded nothing; after the exact notes were released, dispatch 44913249402 attached
the eight assets. Expect this whenever crates.io approval takes longer than the
collector's 600 s wait: create the release only after confirmed publication, then
dispatch on the tag ref with dry_run=false and candidate=false.

## 0.6.1 product patch release

The owner authorized 0.6.1 on 2026-11-22 for four product fixes: publisher idle
limits, batched durable crate progress, nonidentity upstream index drift and
post-prune toolchain reuse. Follow [the authorization](../AGENTS.md#061-product-patch-release-authorization),
[notes](releases/v0.6.1.md) and [candidate evidence](evals/2026-11-22-release-0.6.1.md).
Prepare one code/version PR with a dated changelog entry, both lock root entries,
regressions demonstrating failure before/pass after, requirements and a clean
package audit. Report only after the exact-head Trusted publishing dry run builds
all five targets, the crate, SBOM and single seven-entry SHA256SUMS successfully.
The owner reviews, merges and tags. Do not merge, tag, push to main or poll CI in a
loop. No new dependency is authorized.

Keep this patch additive: existing public structs and strict frozen-plan JSON/IDs
must remain compatible for the same inputs. Compute metadata diagnostics through
an accessor and display them only in CLI/event reports. Run an exact-version,
`--locked` local cargo-semver-checks installation against published 0.6.0 and
require no major changes; no new project dependency or CI job is authorized.

This is the first real release through phase 2. The tag run must build and sign
before the owner's crates-io approval, publish after approval, create and finish the
draft-first nine-asset GitHub Release, then verify publication. No manual release
creation or notes edits are needed. After publication, verify the registry payload,
docs.rs, fresh installation, all nine assets, all eight provenance subjects, SBOM
bindings, attached bundle/store match and public Windows/GNU/musl downloads. Open
a publication record PR that records how each new job behaved on its first real
run, preserving failures/recovery if any. Notify the GUI maintainer only after
0.6.1 is confirmed live. Signing/approval/publication cannot be proved by a PR dry run.

Published 2026-11-22 through [run 44992707987, attempt 2](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/attempts/2).
The [publication record](evals/2026-11-22-publication-0.6.1.md) verifies the complete
first real phase 2 chain and public nine-asset release. Attempt 1 correctly refused
the tag while CI on the merge commit was unfinished; no downstream signing or
publication ran. Wait for green main CI before tagging, then rerun the unchanged
tag/run if this refusal occurs. Signing completed before the owner-approval wait;
no collector recovery or manual release creation was needed. Historical 0.6.0
ordering/recovery evidence above remains unchanged.

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
   with `cargo install rust-mirror --path UNPACKED --version =VERSION --locked --root FRESH_ROOT` and a fresh
   Cargo home. Check `--version`, CLI/refusals and real native publication/lifecycle
   using the installed executable. A source-tree pass is not a package-install pass.
6. Freeze the reviewed candidate, merge normally, verify tree equality and repackage
   at the exact merge. Compare payloads; only VCS metadata may differ. Record the
   archive hash. Packaged README/changelog changes require a refreshed archive and
   candidate identity even when native evidence carries by unchanged code. Require
   owner publication authorization before the workflow runs `cargo publish`.
   The pipeline's `build` job repeats the package audit and packaged install at the
   tagged commit; the manual checks above remain release-PR evidence.

## After publication

- Verify the official crates.io archive against the API/index checksum, non-yanked
  status, exact release VCS and qualified payload; verify docs.rs build and links.
- Perform a fresh actual `cargo install rust-mirror --version =VERSION --locked`
  and installed-command proof. Do not claim registry installation before upload.
- Current releases create the annotated version tag at the exact reviewed merge
  before Trusted Publishing, as described above. Historical owner-machine releases
  tagged after publication; preserve those records without treating that sequence
  as a current fallback. The pipeline's `github-release` job publishes the exact
  committed versioned notes (for example [0.6.0](releases/v0.6.0.md)) as the GitHub
  Release with the crate files; do not create it by hand. Binary archives/checksums
  are attached only through the release workflow under the owner's 2026-11-15
  authorization. Do not change historical note bodies. Releases up to 0.6.0 were
  created by hand and keep their existing assets.
- Verify live tag/body, full `--published` documentation CI, registry/docs agreement
  and clean remote-aligned main. Record evidence in excluded documentation.
- Never move a release tag, overwrite a published crate or rewrite failed history.
  A discovered published-doc defect requires a new version if its package must change.

## Security automation for future releases

The owner approved [the security baseline](SECURITY-BASELINE.md) on 2026-11-15.
All main changes require PRs, actions use full commit pins, security gates apply,
and protected version tags cannot be updated/deleted. This approval does not
authorize another software release or change a published version. Under the owner's
2026-11-16 token-revocation decision, the [Trusted Publishing workflow](../.github/workflows/trusted-publishing.yml)
is the sole publish route, requiring exact main-reachable tagged source and the
owner's crates-io environment approval. Preserve the registered publisher values
in the security guide; the former manual route is historical only.

Starting with 0.5.4, include the Cargo CycloneDX JSON
SBOM to the five-archive profile and the single SHA256SUMS: seven public files,
six manifest entries. Sign provenance inside [the reusable build workflow](../.github/workflows/build-binaries.yml)
and bind the SBOM to each archive with attest-sbom. Keep native generation read-only
in its shared core, signing isolated and uploading contents-write-only. Use the
new reusable signer identity when verifying that new profile; retain historical
signers/profiles for v0.5.1/v0.5.2 without replacing anything.

Run the two-clean-build measurement and retain its actual binary/archive hashes,
metadata diagnosis and unresolved causes. A green measurement job is not a claim
of bit identity. Follow [GitHub's SLSA reusable-workflow design](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/increase-security-rating),
but add no SLSA3 badge until a future approved release actually verifies the new
provenance. SBOMs cover the frozen all-target Cargo dependency graph, not OS runtimes,
SDKs or CI tools. Scorecard activates on main after merge; do not fabricate its score.

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
The authorized current-version v0.5.1 backfill is complete after coordinator merge;
[dated live evidence](evals/2026-11-15-binary-backfill-0.5.1.md) records the archives,
attestations and public Windows/Linux download execution. Other historical-version
backfills require separate approval.

Since 2026-11-21 [trusted-publishing.yml](../.github/workflows/trusted-publishing.yml)
builds binaries on v* tag pushes and as PR/dispatch dry runs (historical releases
used release-binaries.yml). Each production build checks out
the resolved immutable tag commit, uses Rust 1.90.0 and cargo build --release
--locked --target TARGET. A changed MSRV needs an explicit workflow/helper review.
From 0.5.2, five build targets are x86_64-pc-windows-msvc on windows-2025 (windows-latest until 2026-11-21; same image),
x86_64-unknown-linux-gnu built with pinned cargo-libc 0.7.0 targeting glibc 2.17,
x86_64-unknown-linux-musl on ubuntu-22.04, aarch64-apple-darwin on macos-15 and
x86_64-apple-darwin on macos-15-intel. The CLI Linux static variant remains musl. The new dynamic GNU variant starts with 0.5.2;
v0.5.1 remains immutable with four archives and one SHA256SUMS. Install musl-tools only on the Linux build runner for existing C sources;
set CC_x86_64_unknown_linux_musl=musl-gcc and release-only static-CRT RUSTFLAGS.
Fail if file/ldd do not identify static linkage, or ELF headers include INTERP or
NEEDED libraries. Smoke the binary in an empty chroot, then run real DNS/TLS crate
preview/approval/sync and the existing private-CA proxy kill/cancel/resume fixture.
Record linkage, resolver-file availability, exact run IDs and network results.
No glibc runtime floor remains for musl; GNU has its measured dynamic runtime requirements. Kernel/CPU/network policy and musl resolver/NSS
differences remain bounded in the install guide; do not claim every enterprise
kernel or name-service configuration was executed.
Windows release builds alone set RUSTFLAGS to
-C target-feature=+crt-static -C link-arg=/Brepro; do not change repository-wide configuration, normal
CI or development builds. Audit native DLL imports with vswhere + dumpbin
/dependents and the bounded PE parser: fail if no imports are parsed or if any
match ^(vcruntime|msvcp|ucrtbase|api-ms-win-crt-) case-insensitively. Record actual
system DLL imports; no Visual C++ Redistributable is required by these archives.

Each rust-mirror-VERSION-TARGET.zip (Windows) or .tar.gz (Unix) contains one
same-named folder with the executable, original tagged LICENSE and README.txt.
The README records version, exact source commit, usage, verification, unsigned OS
prompts and Linux ABI evidence. Smoke-test --version and protocol-1 --version
before packing and after extraction on each native runner. Collect only the five
expected archives, verify source/target/license/digests and produce sorted
SHA256SUMS with lowercase SHA-256, two spaces and file name. Preserve build reports
as workflow artifacts; record exact run IDs and measured ABI in dated evidence.

The original v0.5.2 workflow used the following permission/profile arrangement;
the newer future-release security baseline above supersedes it. Prepare/build/verify
jobs were read-only. Only the original publish job had contents: write,
id-token: write and attestations: write. It uses actions/attest-build-provenance to
attest all five archives and SHA256SUMS with Sigstore, then gh attestation verify
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

A PR-triggered read-only **candidate** build freezes the exact PR head and reads
its package version and committed notes without creating a tag or GitHub Release.
For explicit pre-tag qualification dispatch the branch with tag=vVERSION,
dry_run=true and candidate=true. The helper accepts an exact candidate commit only
for resolve/package/collect; preflight/upload never accept candidate options.
Prepare refuses candidate=true with dry_run=false, and the privileged publish job
independently requires candidate=false. Candidate runs skip only the unavailable
live release/tag/body check; they still smoke all five binaries, require all eight
GNU gates, collect/verify one six-entry manifest (five archives plus SBOM) and inspect committed notes.
They cannot attest or upload release assets.

Ordinary dispatch dry runs (candidate=false) still build from an existing immutable
tag and check its live release/tag/body, allowing existing assets because they
cannot change them. Live dispatch uses the exact immutable tag workflow ref with dry_run=false and
candidate=false; current main is allowed only when the tag and invocation SHA
coincide. Alternate source refs cannot execute in main-scoped jobs. Tag-triggered publication always uses the immutable tag. Never
create a public throwaway release to qualify a candidate.
For the future SBOM profile, download all seven live assets (eight from v0.6.0, including the provenance bundle; nine after 0.6.0, including the .crate) after uploading, verify SHA256SUMS and attestations,
inspect archive layout/README/source, recheck release body/tag and record evidence.

### Attached provenance bundle (Scorecard Signed-Releases)

Scorecard reads release assets, not GitHub's attestation store. From v0.6.0 every
release therefore also carries `rust-mirror-VERSION.intoto.jsonl`. The signer job in
build-binaries.yml (id-token/attestations write, no contents write) saves the
`bundle-path` output of actions/attest-build-provenance and stages it with
`release_binaries.py stage`, which requires the bundle's SLSA v1 subjects to equal
exactly the signed files. After 0.6.0 those are the .crate, five archives, SBOM and
SHA256SUMS; for 0.6.0 they were the archives, SBOM and SHA256SUMS. `github-release`
(contents write only) re-stages it, checks every asset with
`gh attestation verify --bundle` under the same-commit signer policy, and requires
each attached DSSE envelope (payload and signatures) to equal one returned by
`gh attestation download` from GitHub's store before any upload. SHA256SUMS never
lists the bundle: SHA256SUMS is a provenance subject, so that would be circular, and
the bundle is self-authenticating through Sigstore.

Dry runs and PR candidates cannot sign. The core stages an unsigned placeholder
bundle on a copy of the dry-run assets, which exercises the subject check and the
exact upload set. The read-only download verifier (PR-triggered on its own changes,
default tag v0.6.0) repeats the offline `--bundle` verification and store match on a
real published release. v0.6.0 and earlier are never modified.

See [install verification and unsigned prompts](INSTALL.md). GitHub provenance is
separate from Windows Authenticode/Apple Developer ID or notarization; no paid OS
signing is configured. Keep historical no-assets records dated and intact. New
assets do not change immutable crates.io payloads or the release's source tag.

[Verify published binary downloads](../.github/workflows/verify-release-binaries.yml)
is read-only: dispatch with the tag to download the exact versioned asset profile, check exact notes/
source/license, SHA256SUMS and every GitHub attestation, then run the actual downloaded
Windows ZIP/Linux GNU and musl binaries on native hosted runners. No Rust rebuild/install,
release mutation, write token or OIDC permission is used. API credentials are removed
from the product executable's environment. Save its reports with the release record;
its native-execution scope is Windows/Linux, not post-download macOS execution or
browser quarantine UI.

## Glibc baseline and CI-only compatibility measurements

Owner decision, 2026-11-15, following the measured Alma8 baseline: publish the
cargo-libc 0.7.0 **glibc 2.17** build starting with 0.5.2. Pin the tool
source to e68ca7bf3a8ffd973c8576acc48abd0fbcc66d25. The Ubuntu24.04 release job
installs that source with --locked, uses Rust1.90 and Clang18/GCC13, and runs:

```sh
cargo libc build --target x86_64-unknown-linux-gnu.2.17 \
  --cc /usr/bin/clang-18 --artifact target/x86_64-unknown-linux-gnu/release/rust-mirror \
  -- --release --locked
```

[The GNU producer](../scripts/release_gnu.py) scopes the managed target directory
and tool PATH to this build. It records exact source/tool/compiler/image identities,
binary SHA-256, dynamic NEEDED libraries and maximum required GLIBC symbol; fail
above 2.17. Before packaging, launch and real official HTTPS crate preview/approved
sync must pass in pinned CentOS7 AND AlmaLinux8 userlands. The six other proven
userlands (Leap42.3, Ubuntu16.04/18.04/20.04, Debian8/9) also block on failure.
Packaging independently binds those required gate results to the binary/source hash.
Upload bounded logs and gnu-report.json even on failure; failed gates prevent the
build/verify chain reaching the sole privileged publication job.

The Alma8 image remains pinned to
almalinux:8@sha256:2d0f2eb2a83cc09dbc2eb92fade4dbe8f709628149e81aa00aec70630406603b
for runtime checks and historical baseline experiments, not the selected producer.
Keep static musl unchanged. No old native kernel is qualified by these container
userland checks; glibc below 2.17 should use musl.
[CI-only measurements](../.github/workflows/glibc-experiments.yml) use pinned
rustcommons/cargo-libc, request glibc2.28/2.17 and probe the first lower boundary.
For each build, test old public userland images with version and real plan/sync TLS;
record every combination, including build, pull, loader, DNS/TLS and application
failures. openSUSE Leap42.3 is a community SLE12-era proxy, not native commercial
SLES qualification. No EULA/subscription is accepted or bypassed. Containers share
the CI host kernel and do not prove execution on the original old kernels.

Measurements run on PR opening and explicit dispatch; source/workflow SHA-256s bind
results so documentation-only commits can carry unchanged experiment evidence.
Material experiment changes require a new dispatch. Commit the results/limits to
docs/evals with machine-readable JSON. Report first-party tool bugs/limits to its
maintainer with pinned command/log repros. Do not promote an experiment automatically:
only a proven build may be proposed to the coordinator as the published GNU artifact.
The initial ordinary Alma8 baseline and proposal remain historical evidence. The
owner has now selected the proven 2.17 method above; lower/new candidates remain
CI-only until measured and separately approved.

The five-archive layout starts with owner-authorized 0.5.2, one SHA256SUMS for
all five. Do not backfill GNU into v0.5.1, replace its manifest/assets, add a second
manifest or move its tag. No new crate dependency, version bump or crate publication
is authorized by this pipeline/measurement task.
