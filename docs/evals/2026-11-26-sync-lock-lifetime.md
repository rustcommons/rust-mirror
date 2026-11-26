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

A private shared `WorkspaceLock` explicitly unlocks on drop before closing the
file. Sync, forced sync and selected-download sessions retain it for the same
ownership scope as before. Acquisition failures never construct a guard and do not
unlock another owner. Error exits after acquisition also release explicitly.
Persistent lock markers and fail-fast contention refusal remain unchanged.

Two Linux regressions retain a duplicate open description while the owner exits:

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
  source `ecd62f85d54e20b0137c676767459ac5ddc23886`: Ubuntu both lanes passed
  250/250; Windows results pending when this record was first written.
- Fixed-source dispatch and final PR validation: pending.

The temporary workflow will be removed from the investigation branch and excluded
entirely from the fix PR. Keep #85 open: the historical error is unknown. Observe
normal default-parallel CI for one further month, through 2026-12-26, unless a
new diagnostic establishes the cause earlier. This bounded lock fix does not
qualify hostile mutation, multi-machine/NFS locks, or power-loss behavior.
