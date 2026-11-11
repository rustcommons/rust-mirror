# Local retention validation — 2026-11-11

## Source and scope

Baseline: `18a467508d4db21e64cc8dd35f266d1ad022c63d` (published 0.1.x history).
Implementation revision: `2a38078d446ea818b6998f0eca9993807d590622`.
Validation environment: Linux aarch64, Rust/Cargo 1.90.0, private disposable fixture
roots. No user mirror, Rustup/Cargo client home or real Nexus was pruned. Package
version remains 0.1.1; these source changes are the unreleased 0.2.0 feature, not a
publication or a claim that immutable published 0.1.x archives contain pruning.

## Design checks

Owner-approved protection is declarative because offline clients cannot report
need. Whole-generation deletion preserves immutable collections. Counts are minimum
coverage, not exact per-release compaction: a protected root retains every file.
The current generation is the first explicitly ordered root; order is not inferred.
The administrator must supply the active root and all intended candidates correctly.

Reviewed primary references: [restic retention/dry-run policy](https://github.com/restic/restic/blob/master/doc/060_forget.rst),
[Rust file locking](https://doc.rust-lang.org/1.90.0/std/fs/struct.File.html#method.try_lock)
and [filesystem race limits](https://doc.rust-lang.org/1.90.0/std/fs/index.html#time-of-check-to-time-of-use-toctou).
These inform preview/locking boundaries, not a compatibility or production safety
certification. Deletion unlinks only enumerated files and removes empty approved
directories. Advisory locks do not protect HTTP clients, publishers or hostile edits.

## Focused matrix

`cargo test --locked --test retention`: 20 tests passed locally, including:

- Exact read-only preview, lengths/inventory-byte accounting, explicit API authority,
  exact successful deletion/report and preserved current/enclosing workspaces.
- Additive pins/rollback, missing-pin refusal, semantic numeric ordering, nightly
  inclusive window and zero counts, future-date refusal, leap-day/window boundaries.
- Distinct component variants, equivalent renamed copies, beta explicit protection,
  completed schema-2/3 validation and unchanged kept collections.
- Optional numeric lookup capabilities preserved even when an equivalent raw
  selection exists in the current generation.
- Modified preview, changed kept inventory, corrupt candidate payload and unrelated
  file/empty-directory refusal before deleting any inventory marker.
- Duplicate/overlapping/missing/file/traversal/too-many roots; legacy schema refusal;
  malformed config/policy/moving/duplicate pins and count bounds.
- Busy prune/sync/fetch locks, malformed locks and unlocked retry; default CLI dry-run,
  mode conflict and structured explicit apply.
- Unix linked ancestors/payloads/locks and non-UTF-8 root refusal.
- Unix permission-induced mid-delete failure: successful-prefix report, marker removed,
  current preserved and partial-root replay refused. Permissions restored for cleanup.

The initial component-variant fixture attempted an unavailable default profile and
correctly failed snapshot validation; it was corrected to an explicit redundant
component with a valid minimal-profile closure. This was a test fixture correction,
not a relaxation of the validator.

## Read-only real metadata checks

[Recorded original upstream shapes](2026-11-11-retention-metadata.toml) contain URL,
byte length, SHA-256, original date and compiler release for 10 numeric stable
manifests (1.81.0–1.90.0), dated stable 2025-09-18 and six dated nightlies
2025-09-16–21: 17 successful reads. Dated beta 2025-09-18 returned HTTP 404; that
failed lookup is preserved and not treated as a valid beta sample.

The integration corpus replays these 17 selector/date/release shapes through real
local snapshot construction and retention, checking zero policy, explicit pin and
latest-stable/three-day-nightly-window outcomes. Fixture payloads are synthetic;
upstream archives were not fetched. This verifies data shape and local policy
behavior, not real-archive removal, native installation or production traffic.

## Repository checks and limits

- `cargo fmt -- --check`, `cargo test --locked`: passed (105 Rust tests and 3
  doctests, including the packaged retention example).
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`: passed.
- Python unittest discovery: 16 passed.
- Local release-documentation checker: passed for the two published versions;
  Unreleased changes do not invent a new release or version history.
- `cargo package --locked`: passed from clean implementation revision; unpacked
  verification compiled successfully. Included source/lock/manifest-original/docs/
  examples/license bytes matched the checkout (19 files); VCS metadata was clean.
  Local source archive SHA-256: `eb59fe39488a135788373f82b378fd273b4afc00ec54f2252299b2d7afb5b2f9`.
  This is an unpublished source-check archive, not a 0.1.1 replacement or a 0.2.0 release.
- Existing shared traceability/release/Working/Git sections and CLAUDE shim compared
  byte-for-byte with baseline: preserved. All 31 existing requirement rows unchanged;
  ROM-REQ-032–034 added without reuse.

Local links/anchors and GitHub-flavored Markdown rendering passed for all 12
changed/new Markdown files, including both requirements tables. `git diff --check`
passed. Final platform CI is attached to the PR at its exact head; local Linux
results do not substitute for a Windows run.

This evaluation does not rerun unrelated four-host Rustup/Nexus lifecycle proofs.
No publication or release tag was created. Nexus deletion is deferred: existing
publication cannot prove safe active-group/client/writer protection. No remote
DELETE route is present. Current in-flight readers, power loss, hostile concurrent
filesystem changes, shared-machine/NFS operation, in-generation compaction and
persistent GUI execution plans remain outside this iteration's verification.
