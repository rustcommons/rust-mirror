# rust-mirror

## Project scope

The owner selected `rust-mirror` for the core tool/CLI and `rust-mirror-gui` for a
future graphical interface on 2026-11-09. Planned scope is Rust toolchain and crate
mirroring for offline development, inspired by Panamax. This repository now has an experimental pinned-toolchain CLI/library and a static-host
publication helper. Versions 0.1.0, documentation-only 0.1.1 and feature release 0.2.0 are published;
see docs/evals/2026-11-12-publication-0.2.0.md.
Describe only demonstrated functionality
as implemented; broader mirroring and the GUI remain planned.

Preserve the earlier Windows-local uncommitted prototype. Assess it before import
or replacement; do not delete or overwrite it. Keep `ROM-REQ` identifiers stable
across the rename. Do not publish placeholder crates merely to reserve names.

## Repository layout and validation

- `README.md`: current public introduction and experimental boundaries.
- `docs/REQUIREMENTS.md`: confirmed requirements, sources, evidence, and documentation map.
- `LICENSE`: existing license; preserve it.
- `AGENTS.md`: primary agent guidance.
- `CLAUDE.md`: compatibility pointer to this file.

- `src/`: toolchain selection/planning/downloading core and thin CLI; Rust 1.90+.
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
owner approval before being added. Prepare/qualify 0.3.0 without publishing or creating a
release tag: publication goes through the coordinator after owner confirmation.

Network fixture tests require preinstalled OpenSSL (Git's bundled executable on
Windows) only to generate ephemeral CA/server keys. Runtime uses Rust libraries,
never an external OpenSSL command. Keep fixture keys out of logs/artifacts and
remove their temporary directories. Test proxy auth/exclusions, additive roots,
chain diagnostics, socket deadlines, cancellation/heartbeats, HEAD estimates and
cache corruption/locking. Network config never changes frozen payload identity.

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

- For a repository that ships versioned software, maintain a root `CHANGELOG.md` with an `Unreleased` section and dated, user-facing entries for shipped versions. Record noteworthy changes and their user impact as work is completed; do not substitute a roadmap, Git tag, CI log or validation report for a changelog.
- Before publishing a version, commit its release notes under `docs/releases/vX.Y.Z.md` and link them, the changelog, and the maintained release policy. Explain changes, upgrade or dependency impact, validation and known limits; distinguish software changes from documentation-only releases.
- A software release is not complete until its immutable source tag, applicable registry archives, GitHub Release notes, changelog, documentation, and required verification agree. GitHub Releases document the release; do not attach standalone binaries or other new distribution artifacts without authorization.
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
