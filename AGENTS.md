# rust-mirror

## Project scope

The owner selected `rust-mirror` for the core tool/CLI and `rust-mirror-gui` for a
future graphical interface on 2026-11-09. Planned scope is Rust toolchain and crate
mirroring for offline development, inspired by Panamax. This repository now has an experimental pinned-toolchain CLI/library and a static-host
publication helper. Versions 0.1.0, documentation-only 0.1.1 and feature releases 0.2.0/0.3.0/0.4.0/0.5.0/0.5.1/0.5.2/0.5.3/0.5.4/0.6.0 are published;
see docs/evals/2026-11-21-publication-0.6.0.md.
Describe only demonstrated functionality
as implemented. Crate mirroring shipped in experimental 0.4.0 with bounded qualification;
its postpublication record identifies registry, documentation and installed proofs. Other work
remains governed by the roadmap.

Preserve the earlier Windows-local uncommitted prototype. Assess it before import
or replacement; do not delete or overwrite it. Keep `ROM-REQ` identifiers stable
across the rename. Do not publish placeholder crates merely to reserve names.

## Security baseline and protected refs

The owner approved the organization baseline on 2026-11-15; the coordinator enabled
provider security features, CodeQL default setup, main/release-tag rulesets and the
crates-io environment. **All main changes go through PRs**; never push directly to
main or use administrative bypass for routine work. Preserve lf-only and other CI
gates. Never update/delete protected v* tags, force-push or discard concurrent work.

Pin every direct third-party action to a full commit with its version comment.
Disable checkout credential persistence. Keep workflow defaults read-only and
scope writes to the jobs that need them. Require zizmor and cargo-deny under
[the security guide](docs/SECURITY-BASELINE.md); keep audit exceptions narrow and
justified inline, source restrictions strict, and informational/duplicate warnings
visible. Do not bypass an advisory to make CI green. Document current lock findings
and the source/qualification of fixes. No new crate dependency names are authorized.


Owner amendment, 2026-11-15: version updates are monthly, with two open PRs per
ecosystem; retain grouped minor/patch updates and the seven-day cooldown. Security
updates remain immediate and independent. Dependabot-authored PRs run core checks
only (Ubuntu fmt/test/Clippy, Python tests, cargo-deny, zizmor, lf-only and managed
CodeQL); no package-install, native-client/Nexus, release, reproducibility or glibc
qualification matrices. Keep full manual/main/release qualification available.
Every repository PR-entry workflow cancels superseded runs per PR; never use PR
cancellation to interrupt publication. Refresh old bot branches before fresh runs;
rerunning an old cancelled run retains its old workflow. Never merge bot PRs as
part of this maintenance; majors need separate migration/qualification review.

Future binary profiles use the source-bound Cargo CycloneDX SBOM, shared read-only
build core, reusable build/sign boundary and measured two-clean-build comparison.
Do not retroactively alter published profiles/manifests. No SLSA3 badge or claim
until an approved future release actually verifies the new provenance. Preserve
honest raw reproducibility outcomes and unknown causes; metadata-normalized analysis
is not bit identity. Only the isolated reusable signer gets OIDC/attestation writes;
the asset uploader gets contents write only. The Scorecard job alone also needs
OIDC/security-events writes to publish its metrics on main/schedule, never PR code.

Owner decision, 2026-11-16: the PC crates.io token was revoked. Trusted Publishing
is now the only authorized crates.io publish route; there is no owner-machine or
manual-token fallback. Do not restore/create a long-lived Cargo token or publish
crates from a workstation. Earlier owner-machine procedures below describe historical
releases only. Each new version still requires separate owner authorization.
The owner standard, 2026-11-15, fixes
the workflow path as .github/workflows/trusted-publishing.yml and the environment
as crates-io; preserve those exact registration identities. It requires immutable v* source reachable
from main and the crates-io environment's owner approval, with only that publishing
job receiving its OIDC grant. Never store a long-lived Cargo token or publish a test
version/tag to qualify it. The owner must configure the exact publisher values in
[the guide](docs/SECURITY-BASELINE.md#trusted-publishing-only). This baseline task
authorizes no crate version bump, tag creation, software publication or existing
release mutation. Leave existing Dependabot/user branches untouched; use one PR.

Standard release pipeline, owner-approved 2026-11-21: trusted-publishing.yml is the
Rust Commons [standard pipeline](https://github.com/rustcommons/.github/blob/main/docs/TRUSTED-PUBLISHING.md),
workflow `Trusted publishing` with jobs `gate` (Release gate), `build` (Build),
`attest` (Checksums and attestations), `publish` (Publish to crates.io),
`github-release` (GitHub release) and `verify` (Verify publication). Keep job ids,
display names, order, conditions and per-job permissions in step with the standard;
never rename the file or the crates-io environment. An annotated `vX.Y.Z` tag on a
main commit with green CI starts it. `gate` also keeps this repository's guards:
tag source reachable from main and equal to the tag record, and exactly one
`rust-mirror` crate. `build` packages, audits, SBOMs, installs the packaged crate and
writes `SHA256SUMS-crates`; PR dry runs exercise `gate` and `build` with
publish=false. Only the owner's crates-io approval lets `publish` run. `github-release`
then creates the GitHub Release from the committed notes, so write them in final
form before tagging and never create the release by hand. `verify` checks crates.io,
the exact tag payload, attestations, docs.rs and a clean install. Binaries stay in
release-binaries.yml, which attaches its own files to that release; folding them
into this pipeline is phase 2 in [the roadmap](docs/ROADMAP.md#standard-release-pipeline).


Owner amendment, 2026-11-17: never put `[skip ci]` or `[ci skip]` in commit or
merge messages; preserve CodeQL default setup and normal CI execution. Workflow
installs require full action commits, exact locked Cargo tool versions, hashed pip
requirements (`--require-hashes -r`), digest-bound containers, and a pinned SHA-256
check before executing downloaded installers. No floating download-and-run paths.
The approved `libfuzzer-sys`/`arbitrary` names are fuzz-only: keep `fuzz/` a separate,
unpublished workspace outside the shipped crate dependency graph. Maintain the
[offline parser targets](docs/FUZZING.md), small seed corpora and pinned
ClusterFuzzLite PR/weekly jobs. Crashes require a regression/fix or owner escalation;
never suppress, ignore or retry them into a green result. Bot PRs skip fuzzing.

## Repository layout and validation

- `README.md`: current public introduction and experimental boundaries.
- `docs/REQUIREMENTS.md`: confirmed requirements, sources, evidence, and documentation map.
- `LICENSE`: existing license; preserve it.
- `AGENTS.md`: primary agent guidance.
- `CLAUDE.md`: compatibility pointer to this file.

- `src/`: toolchain/crate selection, planning, downloading and thin CLI; Rust 1.90+.
- `src/crates/`: catalog, frozen previews, controlled sparse-registry operations and crate-aware retention; introduced in 0.4.0.
- `scripts/publish_raw.py`: Python 3.11+ static-host HTTP PUT/GET helper.
- `scripts/prove_offline.py`: Linux network-namespace proof, including negative controls.
- `docs/NEXUS-RAW.md`: actual setup, publication behavior and limits.

Run `cargo fmt -- --check`, `cargo test --locked`,
`cargo clippy --locked --all-targets -- -D warnings`,
`RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps` (set the equivalent
PowerShell environment variable on Windows), and
`python -m unittest discover -s tests -p "test_*.py"`.

The real installation proof downloads about 100-150 MB for the legacy minimal
case; selected tools plus all advertised formats can require several hundred MB.
It requires Linux namespace permissions and an isolated output/evidence directory. Run only the documented
`sudo unshare --net python3 scripts/prove_offline.py ...` workflow; do not weaken
network isolation or replace a clean home with the user's Rustup/Cargo home.
Do not publish artifacts to a real Nexus endpoint without owner authorization.
Never log credentials or turn off TLS verification. Preserve upstream manifest
bytes; check byte/path budgets and hashes before exposing a completed snapshot.
Record actual Nexus/TLS/auth testing separately from the local protocol fixture.
The original proof does not establish independent OpenPGP validation, shared-cache
concurrency or untested-platform support. Current native/package evidence is tracked
separately in docs/evals/2026-11-11-rustup-lifecycle.md; completed publication is
recorded in docs/evals/2026-11-11-publication.md.

The actual Nexus proof is a destructive **disposable-instance test**, not a
production configuration tool. `scripts/run_nexus_proof.py` requires an explicit
already-pulled image and creates only uniquely labeled, network-disabled test
resources. It never accepts server EULAs. `--allow-raw-mime-types` authorizes only
its new test repository's MIME-policy selection; the publisher never changes this
policy. See `docs/NEXUS-INTEGRATION.md` before running it. Legacy OSS test images
must not be presented as recommended production versions. Generated passwords and
private keys must not enter logs, commits or uploaded artifacts.

## 0.1.0 completion scope

The owner requires complete selected Rustup lifecycle and actual Nexus testing for
0.1.0; scheduling is excluded. Follow [release gates](docs/RELEASE-0.1.0-PLAN.md).
Do not substitute metadata availability for Windows/macOS native client evidence,
or fixture HTTP for actual Nexus. Preserve isolated homes, existing generations,
upstream manifests, license gates and documented trust limits. The owner authorized
0.1.0 publication on 2026-11-11; later publications require their own authorization.

## Selection workflow constraints

Keep saved selections strict and hosts explicit. Distinguish host compiler platforms
from compilation targets and metadata availability from native execution evidence.
Use `init`, `list`, `import`, `plan`, then `fetch` as documented in
`docs/SELECTIONS.md`; keep the legacy `fetch-toolchain` route compatible.
`plan` must not download payload archives. `fetch` must use frozen metadata, preserve
original bytes and include every advertised archive format for selected components.
Explicit unavailable selections must fail, not silently shrink or change date.

Validate cached-plan replay, aliases/global components, multi-host closure, byte
budgets, path/hash/schema refusal and existing-output preservation. Exercise the
publisher against both inventory schemas. The real-Nexus harness accepts selected/collection pairs; distinguish the original
schema 1 proof from current expanded results in the lifecycle evaluation.
Do not describe HTTP-fixture evidence as an actual Nexus pass.
Never touch the user's Rustup/Cargo homes. Keep unimplemented scheduling and in-generation compaction,
independent signature validation and foreign-platform execution gaps explicit.

## Native publication validation

`publish` must work from the installed binary without Python. Reuse the full local
inventory/closure checks, refuse credentials in URLs and plain HTTP, disable
redirect/proxy discovery, validate explicit private-CA bundles, check all destination
absences before any PUT and verify each GET hash with the inventory marker last.
Never print auth data or alter Nexus settings. Test legacy/selected/collection,
existing late markers, malformed inputs, redirects, bad GETs, TLS/auth and actual
no-redeploy Nexus. Local sync force must never become remote overwrite authority.

## Resume validation

Keep ordinary fetch's new-output refusal; require explicit `--resume` and exact
frozen-plan identity for existing selected outputs. Never serve `.download` state.
Rehash reusable files, check range offset/length/budgets before appending, verify
whole-file hashes and copied assembly bytes, and never overwrite a completed tree.
Keep the persistent lock file: OS locks, not file existence, determine ownership.

Test ignored/malformed/truncated/oversized ranges, corrupt progress, different plans,
unknown paths/links, competing writers and process termination on Linux/Windows.
Use `docs/RESUMING.md` and `scripts/prove_resume.py` for the bounded real proof;
seed other files from a verified baseline instead of downloading full toolchains
unnecessarily. Keep local-filesystem/power-loss, initial-state and upload limits
explicit. No automatic retries, global cache or resumable legacy route is claimed.

## Multi-version generation validation

`assemble` works only from completed local schema 2/3 inputs. Keep output new and
immutable; retain previous releases and dated metadata. Only explicit new coherent
stable/beta/nightly root triplets may replace previous aliases. Never choose a
winner between conflicting new inputs or silently prune/change bootstrap versions.

Validate every source inventory and selected artifact closure before writing;
hash copy-time bytes and enforce file/record/inventory/final-byte bounds. Test
multi-version retention, alias history, immutable/new-input conflicts, missing
payloads, budgets and symbolic-link refusal. Keep publisher/proof schemas and
requirements aligned. Record actual Nexus schema 2/3 and explicit Raw-group client
activation separately from local assembly. Scheduling remains unimplemented; whole-generation pruning is separate. Use bounded
local fixtures and reused baseline files before downloading another toolchain.

## One-shot sync and demos

For existing sync output, require explicit mutually exclusive `--resume` or
`--force`. Force must discard the old managed progress/mirror without backup, keep writer
ownership throughout reset/restart, and refuse unrelated/link/overlapping
previous paths. Test both platforms, failed restarts and deletion containment. Never
use force to delete previous inputs or bypass locks.

Keep `sync` one-shot and local: no implicit publication, activation, scheduler or
pruning. Freeze the whole cohort before payload transfers; resume must use the same
config/previous identity with no live metadata refresh. Reject incoherent roots and
immutable conflicts. Unchanged runs revalidate the previous mirror, transfer no
payloads and do not create output. Reports are diagnostics, never proof of freshness.

Enforce selected-working-copy and final-generation budgets; retain and validate
private workspace state, lock ownership and expected paths. Test killed-process
recovery, no-op corruption refusal, changed identities, report recovery and untouched
uninitialized directories. Preserve original fetched/assembled behavior when sharing
helpers. Demo scripts must use new private output, distinguish synthetic fixtures
from actual archives, and preserve user homes/inputs. Record counter semantics and
resource/platform limits in `docs/SYNC.md`; no new dependencies without approval.

## Retention and pruning

The owner authorized local retention/pruning for the published 0.2.0 feature on
2026-11-11. Follow [the retention contract](docs/RETENTION.md) and stable trace.
Keep policies explicit: pinned releases, latest stable count, nightly calendar days
and preceding rollback generations. Offline clients cannot discover protection for us.
Keep current first and always protected; preserve distinct protected closures.
For directory-backed GUI plans, list generations/runs without payload verification,
retain the exact next-previous anchor and paused-run dependencies, and bind apply to
managed content/state and membership. Refuse locked/unknown runs; preserve incomplete
and private workspaces. Keep mtime ordering distinct from active routing. General observer/cancel/events support is part of 0.2.0; full workspace cleanup
remains separate work.

Preview must be read-only and return exact file paths/sizes and keep reasons.
Apply requires explicit authorization, writer locks and complete plan revalidation;
return structured successful-prefix/failure results for GUI callers. Never recursively
remove enclosing workspaces, unlisted paths or retained-generation contents.
Test policy boundaries, immutable/schema/closure/path/hash refusal, stale plans,
locks, partial failures, CLI opt-in and both CI platforms. Keep advisory-lock,
whole-generation granularity and interrupted-deletion recovery limits explicit.

No real Nexus deletion is authorized. Remote pruning remains deferred until active
routing/client/concurrent-writer protection can be established and separately approved.
The owner authorized 0.2.0 publication on 2026-11-11 after implementation, merge and
full release qualification. Include retention plus protocol/observer/cancel/typed
errors and no-write publish preflight; follow docs/RELEASING.md.

## Protocol 1 and controlled operations

Maintain [docs/EVENTS.md](docs/EVENTS.md) as the stable CLI/GUI wire contract. Keep
stdout JSON-only in events mode, hello first, one final typed result last and an
independent heartbeat during blocked work. Include stage/operation and progress
sequence; missed heartbeats require the GUI's own watchdog, not optimistic status.
Existing human mode and convenience library calls remain usable. Controlled calls
use shared observers and cancellation tokens; check transfer/copy/hash/delete loops.
Keep initialized frozen fetch/sync state resumable; document new-output and partial
remote/deletion recovery accurately. Signal handlers belong only to the binary and
must perform no I/O/allocation. Do not add dependencies without owner approval.

Preflight must perform no PUT/DELETE, validate local closure and all remote absences,
and distinguish HEAD access from future PUT permission. Publish repeats checks and
verifies uploads. Test JSON framing/escaping, nulls, monotonic sequence, long blocked
I/O heartbeats, stdin record/EOF and signals, cancel/resume and typed TLS/auth/refusal
outcomes. Preserve scoped disposable Nexus and four-host qualification gates.

## Corporate networking for 0.3.0

The owner approved network options, connection diagnostics, opt-in HEAD estimates
and cached official release discovery on 2026-11-12. Follow [the contract](docs/NETWORK.md)
and protocol 1. Keep upstream origins fixed, redirects/proxy discovery disabled by
default, extra CAs additive and all TLS checks enabled. Keep secrets out of persisted
config, argv, debug/errors/events; support programmatic worker credentials. Publishing
uses its independent direct default unless separately configured. No real Nexus
changes are authorized. The owner approved direct `webpki-roots` and `x509-parser` 0.18 with default
features disabled on 2026-11-12, for additive roots and display-only certificate
metadata. Rustls retains all trust decisions. Other new dependencies require
owner approval before being added. The owner separately authorized 0.3.0 publication
on 2026-11-12; the coordinator published from the owner's machine after normal merge.
See the publication record. Later publication still requires its own authorization.

Network fixture tests require preinstalled OpenSSL (Git's bundled executable on
Windows) only to generate ephemeral CA/server keys. Runtime uses Rust libraries,
never an external OpenSSL command. Keep fixture keys out of logs/artifacts and
remove their temporary directories. Test proxy auth/exclusions, additive roots,
chain diagnostics, socket deadlines, cancellation/heartbeats, HEAD estimates and
cache corruption/locking. Network config never changes frozen payload identity.

## Crate mirroring design boundary

The owner approved revised [crate design](docs/CRATES-DESIGN.md) review and native
Phase 0 on 2026-11-12; maintain the [roadmap](docs/ROADMAP.md). Resolution must be
built into rust-mirror: no runtime external/downloaded Cargo and no Cargo library.
Cargo is a CI oracle only. Declared features and one whole-preview approval per plan
are approved. Direct `semver` is approved; earlier `serde_json` approval applies.
No PubGrub/other solver dependency is approved; tar/flate2 is not needed now.
Require bounded native consistency checks, real-Cargo conformance, zero planning
archive downloads and measured overfetch before claiming Phase 0 complete.
The coordinator accepted Phase 0 exit and normal-merged PRs #30/#31. The owner
approved Phase 1 (crate mirroring MVP) on 2026-11-13. Implement the catalog,
approved frozen plans, bounded network fetch/sync/resume, sparse output/schema 4,
retained versions, publication and crate-aware whole-generation protection described
in [the crate contract](docs/CRATES.md). Maintain the protocol additions early and
notify the GUI maintainer when reviewed changes land. Runtime resolution stays native.

Qualification requires roughly the top 100 most-downloaded crates (document any
exclusions), Phase 0 fixtures, isolated Linux AND Windows Cargo HTTPS fetch/build
clients, resume/kill/corruption/budget/yank/links/prerelease cases, measured archive-
byte overfetch and an actual disposable Nexus Raw crate publication/client proof.
Never substitute HTTP fixtures for those named HTTPS/Nexus gates. Use only existing
and already approved dependencies; ask with evidence before adding another. Cargo.lock
import was deferred in Phase 1; the separate 0.5.0 approval below applies.
Coordinator review is required before merging PRs. Phase 1
approval alone did not authorize versioning or publication; the separate 0.4.0
authorization below applies. Production Nexus writes/deletes remain unauthorized.
Preserve existing toolchain evidence and exact common instructions. Historical crate cohorts must retain their original index bytes
and independent proofs, and replay the combined served index across historical and
current profiles; refuse missing edges or resource-bound exhaustion rather than
expose an incomplete generation. Metadata-only inspection/snippet access is not
payload verification. Keep full verification mandatory before publication/pruning.
See [MVP qualification](docs/evals/2026-11-13-crates-mvp.md) for exact source/run
identities; do not generalize bounded Cargo-client evidence to every crate/target.

## Native Windows Nexus qualification

On 2026-11-13 the owner approved native Windows qualification against the official
legacy OSS 3.76.1 win64 bundle and bundled JRE. Use the dedicated
[Windows runner](scripts/run_nexus_windows.py) and [repeatable job](.github/workflows/nexus-windows.yml);
keep the Linux container proof unchanged. Verify the pinned published bundle checksum,
use only fresh marked test state and uniquely named repositories, bind the server/TLS
terminator to loopback and retain TLS verification. Never accept or bypass an EULA;
stop and report if this bundle cannot run without acceptance. No external Nexus target,
production configuration, new dependency, version bump or publication is authorized.

Require real curated roots plus a pinned real project lock with dev-dependencies,
archive-free preview/approval, Windows hard kill during a real archive and exact resume,
preflight with no writes, publication, zero-write repeat and server overwrite refusal.
Use fresh isolated Windows Cargo homes, test-CA-only trust and real HTTPS fetch/build
plus offline build. The optional Rustup stage may add its exact public test CA to
LocalMachine ROOT only on a disposable GitHub-hosted runner, after Cargo's CA-file
proof; require normal/interruption removal evidence. Record its separate bounded
coverage if exercised. Remove only
owned processes/data/keys, refuse reused PIDs and unmarked paths, and upload allowlisted
public evidence only. Keep credentials out of commands/logs/artifacts. A discovered
runtime defect needs coordinator notification and a separate patch-release decision.
See [Windows Nexus evidence](docs/evals/2026-11-14-windows-nexus.md).

## 0.4.0 release authorization

On 2026-11-13 the owner authorized publishing 0.4.0: crate mirroring MVP plus the
accepted contents of main. Prepare the version/lock root entry, changelog, committed
release notes, concise README/docs.rs, applicable qualification and clean package
audit under [the release policy](docs/RELEASING.md). Reuse exact Phase 1 evidence
where runtime code is unchanged; preserve its limits and historical observations.
The coordinator reviews/merges the release PR and publishes from the owner's
machine. After the coordinator confirms publication, complete registry payload,
docs.rs, fresh installation, annotated tag/GitHub Release and verification-workflow
steps, then notify the GUI maintainer that 0.4.0 is live. Do not publish the package
from this checkout or begin GUI Phase 2 on this authorization.
The coordinator published 0.4.0 from the owner's Windows machine after normal
merge on 2026-11-13; see [postpublication verification](docs/evals/2026-11-13-publication-0.4.0.md).
Later versions and GUI Phase 2 still require separate authorization.

## 0.5.0 lock import authorization

The owner approved implementing Cargo.lock import on 2026-11-13. The short
[contract](docs/LOCK-IMPORT.md) was reviewed from the GUI side by the coordinator
and merged in PR #36 before implementation PR #37. Preserve named, digest/time-bearing exact pins and
resolved edges into the self-contained catalog; preserve manual rules and other
sources. Support standard lock formats 1–4, typed bounded diagnostics, official
registry identities only, index checksum matching and lock-scoped yanked permission.
Do not resolve extra closure or fetch git/path entries. Preserve source-aware
preview/explain/retention and all existing security/qualification gates. Prove real
project locks and Cargo clients on Linux/Windows, including old/current formats,
yanks, skips and refusal cases. Existing dependencies only.
Retain typed `keep_sources` lock/manual/pinned reasons so GUI callers do not parse
policy prose. Expose shared bounded `read_input` and `validate_source_name` helpers.
Recognize valid completed crate receipts with an entirely absent mirror as pruned,
not paused; keep private workspaces intact and qualify prune twice on a real chain.
[Iteration evidence](docs/evals/2026-11-13-lock-import.md) names code/results and gaps.

On 2026-11-13 the owner separately authorized publishing 0.5.0 once implementation
is complete and qualified under [the release policy](docs/RELEASING.md), including
release versioning/notes and package audit. Report each green PR to the coordinator;
the coordinator reviews, merges and publishes from the owner's machine. After their
publication confirmation, complete registry/payload/docs.rs/fresh-install/tag/release/
workflow verification and notify the GUI maintainer. Do not merge or publish from
this checkout. This does not authorize GUI implementation or waive any gate.
The coordinator published 0.5.0 from the owner's Windows machine after normal merge
`2bf44ca46f52b2a8974d606eeac0ff6a729c26d1`. The [publication record](docs/evals/2026-11-13-publication-0.5.0.md)
identifies registry payload, docs.rs, installed proofs and release verification.

## 0.5.1 collision patch authorization

The owner approved implementing and, once qualified, publishing 0.5.1 for historical
index build-metadata collisions. The coordinator resumed this work after the owner's
pause; publication remains from the owner's machine after review/merge and green CI.
Do not publish from this checkout. Refuse selected or retained normalized-version
collisions with different checksums using contextual typed diagnostics. Tolerate and
report unselected historical collisions as typed preview warnings, preserving Cargo
candidate exclusions, original full identities and byte-exact served lines. Never
merge checksums or silently substitute an older unambiguous candidate. Require real
Cargo 1.90 full/filtered conformance, pinned ndk-sys metadata, synthetic collision
cases and windowing/Bevy-style regressions. Keep existing safety/resource gates;
no new dependency or runtime Cargo invocation is authorized. See
[collision evidence](docs/evals/2026-11-15-index-collisions.md).
The coordinator published 0.5.1 from the owner's Windows machine after normal merge
`112ef7a7a71d40349c601f299f965199b5b279ba`. The
[publication record](docs/evals/2026-11-15-publication-0.5.1.md) identifies registry
payload, docs.rs, fresh installed proofs, tag/release and verification workflow.
Later publication still requires separate authorization.

## Prebuilt release binaries authorization

The owner authorized unsigned GitHub Release binary attachments on 2026-11-15,
replacing the earlier notes-only distribution boundary. Use the reviewed
[release workflow](.github/workflows/release-binaries.yml) and
[release policy](docs/RELEASING.md#prebuilt-github-release-binaries): exact immutable
tagged source, locked Rust 1.90 builds, native Windows/macOS and static Linux musl smoke tests,
LICENSE/README.txt, SHA256SUMS and GitHub/Sigstore attestations. Under the newer
security baseline the reusable signer alone holds OIDC/attestation writes and the
separate asset publisher holds contents write; native jobs stay read-only. Never replace/delete
existing release assets or change notes/tags to pass an upload check. Keep OS
signing/notarization absent and user prompts explicit. Windows release builds use
static CRT with DLL-import refusal; The Linux static variant uses x86_64-unknown-linux-musl with no
dynamic interpreter/NEEDED libraries, empty-root and real DNS/TLS plan/sync proofs.
Keep these flags scoped to release jobs. No new crate dependency or
crate publication is authorized by this distribution change.

The coordinator merged the workflow; the authorized v0.5.1 dispatch backfill is
complete. Its [record](docs/evals/2026-11-15-binary-backfill-0.5.1.md) identifies
live hashes, attestations and actual native Windows/Linux downloads. Use the
[read-only verification workflow](.github/workflows/verify-release-binaries.yml)
after later asset uploads. Additional historical backfills need separate approval;
future authorized release tags use the automatic workflow. The owner approved a
fifth GNU/glibc archive starting with the NEXT release on 2026-11-15.
After measured CI-only experiments, the owner selected cargo-libc 0.7.0 at
e68ca7bf3a8ffd973c8576acc48abd0fbcc66d25, targeting glibc 2.17, on the same date.
Build with Rust 1.90/Clang 18 using the qualified method; require launch and real
HTTPS plan/approved-sync passes in pinned CentOS 7 and AlmaLinux 8 userlands before
publication. Keep the other six proven userlands as blocking gates too. Keep static
musl unchanged. Record compiler/linkage/image/source identities and bounded results.
Containers prove userlands only: old native kernels are untested, and glibc below
2.17 should use musl. Keep measurements and their initial Alma8 baseline/proposal
historical; this later approval selects the production method.
v0.5.1 stays at four archives with its original single SHA256SUMS; no GNU backfill
or secondary manifest is permitted. Future releases have one SHA256SUMS covering
all five archives. Report cargo-libc bugs/limits to its maintainer. This decision
approves the distribution method, not a new version or crates.io publication.
Do not create throwaway
public releases. Preserve dated notes-only publication
records as historical observations and record new binary distribution separately.

## 0.5.2 release authorization

On 2026-11-15 the owner authorized publishing 0.5.2 to ship five prebuilt downloads:
Windows static CRT, GNU/Linux via the selected cargo-libc 0.7.0 glibc 2.17 method,
unchanged static musl and macOS arm64/x86_64, with one SHA256SUMS and attestations.
Follow [the release policy](docs/RELEASING.md) and [0.5.2 notes](docs/releases/v0.5.2.md).
There is no mirroring runtime logic or dependency change from 0.5.1; only the
reported version and packaged user documentation change. Reuse historical native/
Nexus/conformance evidence only where source bytes and dependency identities match.
Require a clean package audit and green exact-PR-head release-binaries candidate
dry run, including all eight GNU userland launch/real HTTPS plan-sync gates.
Candidate builds must never tag, create a release, attest or upload release assets.
The coordinator reviews/merges and publishes from the owner's machine. Only after
publication confirmation create the annotated exact-merge tag and committed GitHub
Release notes, let the tag workflow attach the five archives plus one SHA256SUMS,
verify all six live assets/attestations and execute downloaded Windows/GNU/musl
archives. Open the bounded publication record PR. Never change v0.5.1 assets.

The coordinator published 0.5.2 after normal merge. Its annotated tag, exact notes,
five archives/one SHA256SUMS, six attestations and actual Windows/GNU/musl downloads
are verified in [the publication record](docs/evals/2026-11-15-publication-0.5.2.md).
Preserve the earlier candidate and v0.5.1 evidence; later versions still need their
own authorization.

## 0.5.3 security release authorization

On 2026-11-15 the owner authorized 0.5.3: the merged PR #51 RustSec lock fixes plus
existing main, with no additional application implementation changes. Follow
[0.5.3 notes](docs/releases/v0.5.3.md) and the release policy. Qualify the clean
package and exact-head five-target dry run with six hashed payloads (five archives
plus SBOM), one SHA256SUMS and all eight GNU gates. List every upstream advisory
and keep configuration-specific exposure, unknown exploitability and logging facts
honest; do not describe TLS/official-origin checks as eliminating trust-chain bugs.

The coordinator merges the green release PR. Then create the annotated v0.5.3 tag
at that exact normal merge; trusted-publishing.yml waits for owner approval in
crates-io, obtains only its short-lived OIDC token and publishes. Do not use a
long-lived token or the owner-machine route for this release. Create exact committed
GitHub Release notes after registry confirmation so binary automation can attach
its seven-file profile. Verify registry/payload, docs.rs, fresh install, all
checksums/provenance/SBOM bindings and public Windows/GNU/musl execution; record in
a PR. Release preparation authorized advisory drafting only; publication remained
with the coordinator. Its completed publication is recorded below.
Do not tag/publish before merge, move protected tags, change existing assets or
claim SLSA3 until the new live provenance is verified.

The owner approved the crates-io environment and Trusted Publishing published
0.5.3 from the exact release merge. See the
[publication checkpoint](docs/evals/2026-11-16-publication-0.5.3.md) for verified
registry/docs.rs/installation evidence and the failed SBOM-attestation dispatch.
Binary upload and public-download verification remain incomplete; do not claim the
seven-file profile or SLSA3 yet. Preserve both failed workflow attempts. The
protected release tag cannot absorb a workflow fix. On 2026-11-16 the owner
rejected a recovery exception and authorized 0.5.4 instead, as recorded below.
Version 0.5.3 remains crate-only. The coordinator subsequently published the
repository advisory after 0.5.4 verification, as recorded below.

## 0.5.4 download security release authorization

Owner decision, 2026-11-16: no recovery exception or binary backfill for 0.5.3.
That published crate remains available without prebuilt downloads; preserve its
immutable tag, notes, failed workflow history and empty asset profile. The release
policy requires historical GitHub note bodies to match their tagged records, so
leave 0.5.3 notes untouched and explain the outcome in maintained docs/0.5.4 notes.

The owner authorizes 0.5.4 with the same application code and locked dependencies
as 0.5.3, the merged deterministic SBOM serial fix and existing main. Only root
version/user documentation changes are needed. Follow [0.5.4 notes](docs/releases/v0.5.4.md)
and [the release policy](docs/RELEASING.md#054-download-security-release).
Require a clean package audit and exact-head five-target/seven-file candidate dry
run with all eight GNU gates. Exercise SBOM generation/final validation/collection;
dry runs cannot claim actual OIDC signing or live SBOM bindings.

The coordinator reviews/merges the green release PR. Then tag v0.5.4 at the exact
normal merge; Trusted Publishing waits for the owner to approve crates-io. After
confirmed publication, create the exact committed GitHub Release, run tag-bound
binary automation and verify registry/source, docs.rs, fresh install, seven assets,
new reusable signer/SBOM bindings and public Windows/GNU/musl execution. If the
collector wait expires, only the existing exact-tag dispatch path is allowed,
with no assets present. Never relax source/signer guards or use a different builder.
Record publication in a PR. Advisory publication was reserved for the coordinator:
affected <=0.5.2, patched crate 0.5.3, first patched downloads 0.5.4.

Owner-approved Trusted Publishing and the tag-bound seven-file upload completed.
The [publication record](docs/evals/2026-11-16-publication-0.5.4.md) verifies all
provenance/SBOM bindings, docs.rs, registry installation and public Windows/GNU/musl
execution. The initial Windows reader CP1252 failure and explicit UTF-8 correction
remain recorded; the producer, tag and assets never changed. Keep strict exact
predicate matching and distinguish reader revisions from immutable artifact source.
The release-specific L3 badge follows verified live reusable provenance, not an
independent certification or a claim about older signers. The coordinator published
[GHSA-46c8-w6pp-97pw](https://github.com/rustcommons/rust-mirror/security/advisories/GHSA-46c8-w6pp-97pw) at 2026-11-16T05:35:02Z. Preserve dated draft
observations as history; current status is published.

## 0.6.0 dependency release authorization

Owner authorization, 2026-11-21, applied as 0.6.0 by coordinator decision the same
day: release existing main,
including merged PR #72: sha2 0.11, toml 1.2, a direct webpki-roots 1.0
requirement (ureq still pulls the 0.26 shim), serde 1.0.229, tempfile 3.27 and
the matching fuzz workspace. rustls 0.23.46 and rustls-webpki 0.103.17 stay as
locked in 0.5.4. No other application change and no new dependency. Follow
[0.6.0 notes](docs/releases/v0.6.0.md) and
[the release policy](docs/RELEASING.md#060-dependency-release). It is a minor
release, not 0.5.5, because the public `Error::report` field exposes `toml::Value`
and toml moves from 0.8 to 1: a SemVer break for library users. The owner allowed a
minor release when a change requires it (2026-11-15). The upgrade notes must state
the break plainly. No 0.5.5 is planned.

Require a clean package audit and an exact-head candidate dry run: five archives
plus SBOM, one six-entry SHA256SUMS, all eight GNU userland gates and the
provenance-bundle staging. 0.6.0 is the first release that attaches
rust-mirror-VERSION.intoto.jsonl; dry runs cannot sign, so its bundle is verified
only after the tag. Report the green PR and dry-run ID to the coordinator. Do not
merge, tag or publish; never push to main or move v* tags.

After the coordinator confirms crates.io publication: check the registry payload,
docs.rs and a fresh install; create the exact committed GitHub Release; let the
tag-bound workflow upload eight assets and verify every attestation, SBOM binding
and the attached bundle against GitHub's store; run the public Windows/GNU/musl
download verifier. Record publication in a PR, then tell the rust-mirror-gui
maintainer that 0.6.0 is live. The same exact-tag-only recovery rule as 0.5.4
applies to a collector timeout.

Published 2026-11-21. The tag run's collector timed out (HTTP 404) because owner
publishing approval came after its bounded wait; it skipped attest/publish and
uploaded nothing. After the exact notes were released, exact-tag dispatch 44913249402
attached all eight assets. The [publication record](docs/evals/2026-11-21-publication-0.6.0.md)
verifies registry payload, docs.rs, fresh install, all seven provenances, five SBOM
bindings, the attached bundle's offline verification and store match, and public
Windows/GNU/musl execution. Keep the failed run as history; never treat a collector
timeout as permission to change source, signer or tag.

## User-facing documentation

The owner authorized docs-only 0.1.1 publication on 2026-11-11. Keep README and docs.rs concise: purpose/install/short quick start and links to
maintained guides; every exported item still needs meaningful rustdoc. Maintain
`src/guide.md` as the packaged docs.rs walkthrough included by `src/lib.rs`. Every
exported item/field needs meaningful rustdoc. Keep missing-doc warnings enabled;
compile examples and deny rustdoc warnings during release checks. Public README/API
pages explain usage, not internal coordination/prototype history.


Maintain `docs/USER-GUIDE.md` as the plain-language start-to-finish route and
`docs/CLI-REFERENCE.md` as the complete command/configuration reference. Document
every command/option's meaning, defaults, output, network/local/remote effects and
failure/retry behavior. Explain host versus target, compiler versus manager,
raw versus finished generation, and publication versus explicit activation.
Keep Windows and Unix client examples, safe credential/trust handling and known
limits aligned with implementation; never imply all historical/custom Rustup
selectors or unqualified hosts are supported.

For documentation changes, audit CLI/source coverage, parse configuration/shell
examples, compile changed Rust API examples, check links/anchors/rendering and
inspect the diff. Do not repeat unrelated native matrices for unchanged code.
When packaged README/changelog bytes change, refresh the clean package audit and
release-candidate identity; do not publish an earlier archive missing the new docs.
Preserve dated qualification history and explicitly carry only applicable evidence.

## Documentation and requirements traceability

- Follow the [Rust Commons documentation conventions](https://github.com/rustcommons/.github/blob/main/docs/DOCUMENTATION.md).
- Maintain [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md) as the repository's index of confirmed requirements, their sources, implementation or documentation, verification evidence, and gaps. Preserve stable repository-scoped IDs and existing identifiers; never renumber or reuse them.
- Distinguish verified behavior, partial verification, planned work, deferred work, and scope exclusions. A source reference or passing test supports only the scope it actually checks; record missing evidence explicitly.
- Keep the README focused on purpose, prerequisites, setup, and guide links. Keep detailed workflows in maintained guides, future work in the roadmap, and dated release or validation evidence identifiable as historical records.
- Update affected requirements, documentation, and trace links together when behavior, scope, interfaces, prerequisites, or validation changes. Use exact versions or revisions for release evidence, and do not rewrite historical outcomes or treat repository edits as changes to an immutable published package.
- Validate documentation changes with source and command checks, links, rendering, and a clean diff. Run additional builds or tests only when changed examples, package contents, or behavior require them.

## Release documentation

- **Updating `CHANGELOG.md` is critical.** It is the source the release notes are written from, and the release gate refuses to publish without a dated `## [X.Y.Z] - YYYY-MM-DD` entry and `docs/releases/vX.Y.Z.md`. Add an `Unreleased` entry in the same PR as each noteworthy change, never later; a release PR only dates and moves those entries.
- For a repository that ships versioned software, maintain a root `CHANGELOG.md` with an `Unreleased` section and dated, user-facing entries for shipped versions. Record noteworthy changes and their user impact as work is completed; do not substitute a roadmap, Git tag, CI log or validation report for a changelog.
- Before publishing a version, commit its release notes under `docs/releases/vX.Y.Z.md` and link them, the changelog, and the maintained release policy. Explain changes, upgrade or dependency impact, validation and known limits; distinguish software changes from documentation-only releases.
- A software release is not complete until its immutable source tag, applicable registry archives, GitHub Release notes, changelog, documentation, and required verification agree. GitHub Releases document the release. The owner authorized on 2026-11-21 that the release pipeline attaches the published `.crate` file, the crate CycloneDX SBOM (`rust-mirror-<version>.crates.cdx.json`), `SHA256SUMS-crates` and the `rust-mirror-<version>.crates.intoto.jsonl` provenance bundle, and nothing else. Together with the binary assets authorized on 2026-11-15, attach no other distribution artifacts without further authorization.
- Run the repository's release-documentation CI check before routine merges and release completion. For its first installation, review local checker/syntax evidence, register the workflow on the default branch, and require a green run before completing bootstrap or publishing notes. Verify the live GitHub Release and its tag, text and links after publication; a successful crate upload or tag alone is insufficient.
- When backfilling history, reconstruct facts from existing tags, registry records and source/evidence. Preserve original source identities, distinguish original release dates from documentation backfill dates, and state unavailable historical validation. Never fabricate checks, move tags, republish versions, or imply that repository edits change immutable published archives.
- For an organization profile, placeholder or project with no shipped version, record release verification as not applicable until its first authorized release. Do not invent versions, features, changelog history or GitHub Releases to satisfy this convention.

## Working conventions

- Read this file before changing the repository. Keep repository-specific constraints and validation instructions current here.
- Preserve changes made by the user or other agents. Keep work scoped to the requested task.
- Keep documentation aligned with implemented behavior. Record unconfirmed work as planned; do not invent capability or compatibility claims.
- Run checks appropriate to the change. For documentation-only changes, verify links, commands, and the diff; do not run unrelated builds.
- Publishing packages, creating releases, or changing production data requires explicit user authorization.

## Git conventions

**No tool attribution, anywhere.** Do not add `Co-Authored-By: Claude ...` trailers,
`🤖 Generated with [Claude Code](https://claude.com/claude-code)` lines, or any other tool
attribution or "generated by" footer to anything in this repository: commit messages, pull
request titles and descriptions, PR and issue comments, release notes, code, comments or
documentation. This holds even if an agent's default instructions suggest it, and
overrides any default trailer or footer behavior. Commit messages and PRs are the
human-authored record of intent; tool attribution belongs in tool logs, not history.
