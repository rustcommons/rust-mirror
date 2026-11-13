# Cargo.lock import iteration — 2026-11-13

## Scope and source

The owner approved implementation, then separately authorized 0.5.0 publication
once ready. The coordinator reviewed the GUI contract and merged PR #36 at main
`e18f6f852b2af9355727d5f19bf8e0a161380e4d`; main CI
[43463590072](https://github.com/rustcommons/rust-mirror/actions/runs/43463590072)
passed. This iteration includes the requested structured lock-source retention
reasons, bounded file/name helpers and the reported published-0.4.0 prune-twice bug.
The coordinator reviews/merges and publishes from the owner's machine. This record
is candidate evidence, not a registry upload or completed release.

Initial runtime source `3f95e92060f7f89257e756bef79045e85af6e848` introduced the
implementation and real-project harness. [Initial Linux evidence](lock-import-initial-linux.json)
is exact to that source. Later hardening rejects mixed checksum encodings and
ambiguous local references, normalizes the two official source spellings, prevents
malformed-name credential echo and validates external patched edges without fetching
them. Release candidate code/version/package identity and final CI are separately
identified on the implementation PR and package audit; do not treat the initial
0.4.0-version development binary as the published 0.4.0 crate.

## Parser, integrity and retention

The first isolated three-generation chained prune test reproduced
`paused run previous generation is missing from discovery` after successful removal.
The correction recognizes only a valid completed receipt plus an entirely absent
mirror as `pruned`; it preserves plan/private receipt files. Partial mirrors,
unfinished receipts, unknown paths and active writer locks remain conservative.
The same test now previews/applies twice and checks both pruned rows/private receipts.

Focused fixtures cover standard formats 1–4, metadata checksums and missing/invalid/
conflicting checksums, source aliases, abbreviated/ambiguous references, unsafe
registries/names, truncation/cancellation, canonical round-trip, explicit CLI preview/
write/removal and manual-rule preservation. Locked index checks fail before archives;
new manual yanks remain refused. Exact copied source protection covers the entire
pin/checksum set, with typed `keep_sources` reasons. Apply reconstructs both shared
and structured reasons and refuses tampering/stale disk state.

The first CLI shape test caught `invalid-command` hello labels for import/removal;
the dispatch whitelist now emits the documented labels. Two frozen fixtures created
with the actual installed 0.4.0 binary preserve original schema-1 digest reconstruction,
including two historical cohorts; schema-4 payload loading/fetch remains supported.
New schema-2/5 output does not grant old readers unknown semantics.

## Real projects and exact coverage

The [corpus](../../tests/fixtures/crates-lock-corpus.json) pins real source commits,
lock SHA-256, source/license links and explicit build selections:

| Project | Lock format | Build scope |
| --- | --- | --- |
| ripgrep 12.1.1 | v1 | Default program build |
| rust-analyzer 2020-06-01 | v2 | Initially `stdx`; strengthened candidate gate builds `ra_hir_ty`, including Git/path dependencies |
| ripgrep 15.1.0 | v4 | Default program build |
| fd 10.4.2 | v4 | Default program build |

They contribute exactly 326 unique registry versions, 54,373,445 verified archive
bytes, with no lock-source closure expansion. Initial observations include five
yanked versions from rust-analyzer (crossbeam-deque 0.7.3, crossbeam-queue 0.2.2,
miow 0.2.1, net2 0.2.34 and smallvec 1.4.0). All original index lines are retained.
An early detector mislabeled the old ripgrep lock v2 because it interpreted local
source omission as v2; source omission on local references is valid v1. The corrected
fresh committed harness detects v1 and matches all four expected formats.

At source 3f95e92, real Cargo 1.90 on Linux/aarch64 fetched each project from the
verified HTTPS mirror and built offline with unchanged locks in isolated homes.
The mixed-source case provisions Git independently, removes its registry cache,
then fetches registry data from the mirror. A registry-populated home without Git
refuses offline; missing/corrupt registry payload controls refuse without fallback.
The additional `ra_hir_ty` offline build passed too, while old upstream Chalk code
reported future-compatibility warnings. No upstream project was patched to hide
these warnings or alter its lock. Non-build HomebrewFormula links are not extracted
from the ripgrep source archives; every such omission is listed in the corpus.

The backend successfully shows/fetches its saved plan after original lockfiles are
renamed away, then the harness restores them solely for actual Cargo clients.
Runtime code uses no Cargo process/library or new dependency. Existing corporate
network/TLS/secret and native manual-closure behavior remain in force.

## Acceptance gates and limits

The Linux/Windows lock-import workflow downloads the pinned sources, checks lock
bytes, imports transactionally, verifies the exact pin union/zero planning archive
GETs, hides originals during replay/fetch, then runs real HTTPS fetch --locked and
build --locked --offline plus refusals. Its source-stamped summaries identify exact
platform/compiler/build scope. All existing core, native/top-100, three-platform
client/resume, actual disposable Nexus and four-host Rustup gates remain mandatory;
final exact-head runs and artifacts are attached to the implementation PR.

The initial full local suite at 3f95e92 passed 179 Rust tests plus one rustdoc
example, 26 Python tests, fmt, Clippy all targets and rustdoc with warnings denied.
Hardening adds targeted refusal cases; final candidate CI/package checks identify
the final counts separately. Link/render/source checks cover documentation changes.

This proves the named registry pin sets and selected client builds, not every
workspace package, feature combination, SDK or future Cargo/compiler behavior.
Lockfiles do not provide feature/dependency-kind profiles or Git/path contents;
those sources remain explicit client prerequisites. Actual Nexus crate consumption
retains its Linux-only disposable-instance scope; it is not production approval.
Whole-generation pruning does not reclaim private workspaces or compact records.
No publication, tag, registry install or 0.5.0 docs.rs success is claimed here.

## Clean candidate package and installed operations

Candidate source `21346ecb69e4ddda3c22d0ab8e4344aed1253345` bumps the root
version to 0.5.0 without dependency changes. A clean detached-worktree
`cargo package --locked` passed with Cargo 1.90.0. Its unpublished archive SHA-256 is
`997e58046547f89cf338b5204fac3a43582db397c3a432f5767e129c0e21e4f1`: 42
byte-matched source files plus Cargo's two generated metadata files. The VCS record
names that exact clean commit. The [audit](lock-import-candidate-audit.json) records
source hashes; this is candidate evidence, not a crates.io checksum.

Fresh installation from the unpacked archive into new Cargo/install homes reports
0.5.0 and protocol 1. That installed binary imported all four source locks, planned
and reused all 326 verified archives with zero transferred bytes, and returned all
four structured lock-source protections in a non-deleting retention preview.
Separately, all four real Cargo HTTPS/offline client cases were rerun against the
earlier verified mirror, including `ra_hir_ty`, with the candidate harness. This
client-only reuse does not claim that the installed binary downloaded that mirror.
The first client-only rerun stopped before Cargo because the helper expected the
wrong prepared-project layout; the corrected helper passed without product changes.
Fresh mirror creation by each platform's candidate binary remains a CI gate.

The first PR Windows lock-client run (43482618572) passed the old ripgrep case
but refused GitHub's chain while provisioning Chalk: Cargo/libgit2 applied the
fixture-only `http.cainfo` to that separate public origin. The harness now provisions
Git with default public roots before configuring the private mirror CA, discarding
all registry data from provisioning before the actual mirror fetch. Verification
remains enabled for both steps. This harness correction changes no packaged source.
