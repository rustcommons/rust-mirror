# Sync operation lock-lifetime investigation — 2026-11-26

## Incident and bounds

Owner-directed investigation of [#85](https://github.com/rustcommons/rust-mirror/issues/85),
at main `3885f6d255592171a40c84f6292c4540cff9e787`. The historical failure in
`incomplete_assembly_is_rebuilt_but_a_foreign_staging_file_is_preserved` discarded
its error. #86 improved diagnostics; the old log cannot establish its cause.
The [earlier fixture investigation](2026-11-16-sync-lock-fixture.md) identified
fork-inherited lock lifetime in a different test; its fixture-only fix stays valid.

## Review findings

The target test uses a fresh private tempfile directory and in-memory, deterministic
payloads. Resume replays saved plans; the final resume forbids live metadata. No
mtime, wall-clock or directory-listing order selects an assembly result. Collection
inputs, paths and inventory identities use ordered collections. Staging validation
allows only expected files/directories and refuses `foreign` before deletion. Copy
and hash validation is synchronous; copy handles close before the assembly rename.
There is no background downloader or reused temporary path in this test.

A real lifetime race exists at the workspace locks: sync and selected-download
sessions previously relied on dropping a `File` to release its OS lock. Linux
flock ownership follows the open file description. An unrelated thread's fork can
retain that description until exec even with close-on-exec set. Closing the parent's
handle does not release the lock until every inherited/duplicated copy closes.
An immediate resume can therefore see `Locked` after the previous call returned.
The sync test binary has other tests that spawn a crash child or `mkfifo`; its
normal parallel run can overlap those forks. The requested isolated-test stress
runs do not run those sibling tests, even with libtest's default thread setting.

This proves a possible failure mechanism, not the identity of the inheriting child
or the error in the historical #85 run. No foreign-file deletion or mutual-exclusion
violation was observed.

## Fix and deterministic proof

The initial toolchain prototype used a private `WorkspaceLock` in the resume
module. Owner review extended it to [the shared crate-internal module](../../src/workspace_lock.rs).
All workspace and catalog locks now explicitly unlock on drop before closing the
file, retaining their original ownership scope, field position and drop order. Acquisition failures never construct a guard and do not
unlock another owner. Error exits after acquisition also release explicitly.
Persistent lock markers and fail-fast contention refusal remain unchanged.

The initial two Linux regressions retain a duplicate open description while the owner exits:

- Sync refuses another writer while the owner lives, then reacquires immediately
  after the owner returns, with the duplicate still alive.
- An error after acquisition also permits immediate reacquisition while a duplicate
  remains alive.

The sync regression fails on unchanged production locking with typed `Locked`.
Both regressions fail when the guard uses the previous close-only behavior and
pass with explicit unlock. No sleeps, lock retries, serialization, unsafe code or
new dependencies are needed. The original foreign-file recovery test also passes.

## Hosted stress evidence

The temporary branch-only dispatch workflow runs only the named integration test,
250 times per OS/thread-setting lane: 500 each on ubuntu-24.04 and windows-2025.
Every failure retains complete stdout/stderr; JSON artifacts contain every iteration's
exit code and duration. It compiles once per lane with Rust 1.90.0 and runs the same
executable each time; every test invocation creates its own tempfile directory.

- [Baseline dispatch 45842723872](https://github.com/rustcommons/rust-mirror/actions/runs/45842723872),
  source `ecd62f85d54e20b0137c676767459ac5ddc23886`: both lanes passed
  250/250 on each OS (500 Ubuntu, 500 Windows; zero failures).
- [Fixed-source dispatch 45843395105](https://github.com/rustcommons/rust-mirror/actions/runs/45843395105),
  source `2a36e2c`: both lanes passed 250/250 on each OS (500 Ubuntu,
  500 Windows; zero failures), and both Linux lock-lifetime regressions passed
  in each Ubuntu lane. The fix PR carries identical Rust source/test bytes.

The [machine-readable summary](sync-lock-lifetime-20261126.json) records all eight
lane counts and the deterministic red/green controls. Baseline and fixed dispatches
total 2,000 successful isolated invocations; no reproduction of the historical
flake was observed. Initial toolchain-prototype local Linux/aarch64 Rust 1.90 qualification passed the full
Rust suite, Clippy with warnings denied, rustdoc with warnings denied and all 146
Python tests.

The temporary workflow is removed from the investigation branch and excluded
entirely from the fix PR. Keep #85 open: the historical error is unknown. Observe
normal default-parallel CI for one further month, through 2026-12-26, unless a
new diagnostic establishes the cause earlier. This bounded lock fix does not
qualify hostile mutation, multi-machine/NFS locks, or power-loss behavior.


## Whole-class review extension and acquisition audit

The two stress dispatches above qualify the initial toolchain prototype; they do
not substitute for the expanded regressions. Owner review requested coverage of
all remaining workspace/catalog owners and shared retention probes. The production
audit found these eight acquisition paths:

| Path | Ownership |
| --- | --- |
| `src/resume.rs`: `Session::open` | Toolchain fetch/resume `.fetch.lock`. |
| `src/selection/sync.rs`: `lock` | Toolchain sync/force `.sync.lock`. |
| `src/crates/mirror/fetch.rs`: `Session::open` | Crates fetch/resume `.crates.lock`. |
| `src/crates/mirror/sync.rs`: `sync` | New crate preview `.crates.lock`; approved/resumed sync delegates to fetch. |
| `src/selection/retention.rs`: `lock_file` | Prune/generation `.rust-mirror-prune.lock` and existing sync/fetch/crates writer locks. |
| `src/crates/catalog.rs`: `Catalog::write` | Catalog writer sidecar `PATH.lock`. |
| `src/releases.rs`: `load` | Release-cache `.releases.lock` during refresh/fallback. |
| `src/selection/retention/catalog.rs`: `busy` | Shared probes of `.sync.lock`, `.fetch.lock`, `.crates.lock`. |

Every path uses `workspace_lock::WorkspaceLock`; only its exclusive/shared
constructors call `File::try_lock` / `File::try_lock_shared`. Failed acquisition
never constructs an owner and never calls unlock. Existing lock validation,
nonblocking error kinds/messages, read-only probe handles, acquisition order,
struct field positions and explicit catalog-writer drop point are preserved.
The retained `Vec` of prune locks retains its original element/drop order.

Run the deterministic Linux controls with
`cargo test --locked --lib inherited_description -- --nocapture`.
Nine tests retain actual acquired descriptions past return.
They cover completed crates fetch followed by immediate resume, preview followed
by approval, retention probe followed by actual approved crate sync, all three
probe marker types, generation/prune lock reacquisition, catalog replacement,
release-cache refresh, and the original toolchain normal/error returns. A
thread-local test-only capture duplicates successful acquisitions; production
builds contain no capture code. Copies remain alive during the next operation;
there are no timing sleeps, retries or real forks in these controls.

With only `WorkspaceLock::drop`'s unlock disabled (the old close-only behavior),
all nine tests fail; restoring it passes all nine. Both requested end-to-end cases
fail with `Locked: another crate writer owns this workspace` under close-only
release. Live exclusive owners still block shared probes and competing writers.
The [machine-readable summary](sync-lock-lifetime-20261126.json) keeps these
expanded controls separate from the initial two-test proof and stress evidence.

The audit also covers test fixtures: eight intentional raw file-lock owners in
`tests/crates_catalog.rs`, `network.rs`, `resume.rs`, `retention.rs` and `sync.rs`
now explicitly unlock before close. Other `.lock()` calls in `src` are standard
mutex/condition-variable or stdin/stdout/stderr guards, not file locks; they do
not share this close-only descriptor lifetime. No new dependency, unsafe code,
public API or version change is introduced.
