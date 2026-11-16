# Sync fixture lock-release investigation — 2026-11-16

## Incident and scope

[Issue #67](https://github.com/rustcommons/rust-mirror/issues/67) records the exact
failure and findings. The failed test was
unknown_entries_locks_and_nested_outputs_refuse_without_touching_user_files,
[tests/sync.rs:182 at source 192bc5f](https://github.com/rustcommons/rust-mirror/blob/192bc5fe128055ae28b0f84d9c5ef6bd6b33490a/tests/sync.rs#L182).
[Run 43864342715 attempt 1](https://github.com/rustcommons/rust-mirror/actions/runs/43864342715/attempts/1),
[Ubuntu job 133288918426](https://github.com/rustcommons/rust-mirror/actions/runs/43864342715/job/133288918426),
failed at 2026-11-16T12:13:47Z; Windows was cancelled by matrix fail-fast.
The runner was ubuntu-latest, Ubuntu 24.04.5, image ubuntu-24.04 / 20261111.127.1,
agent 2.338.2, Rust 1.90.0. The selected sync suite had 19 passes/1 failure:

```text
thread 'unknown_entries_locks_and_nested_outputs_refuse_without_touching_user_files' panicked at tests/sync.rs:182:68:
called `Result::unwrap()` on an `Err` value: Error { kind: Locked, message: "another sync owns this workspace, or file locking is unavailable", report: None }
```

The panic follows drop(f) on a fixture-held lock, not failed acquisition of the
fixture's own lock. Attempt 2 passed both operating systems with unchanged source;
that historical rerun is not the causal proof or the fix. Baseline main
[CI 43866330803](https://github.com/rustcommons/rust-mirror/actions/runs/43866330803)
passed at de622a3126d34fb7a17401cabebb969fbcf43ebd before investigation.

## Mechanism and deterministic evidence

The failing test uses a private tempfile directory and has no sleeps or readiness
poll. Other tests in the same default-parallel binary spawn mkfifo and a crash/
resume child process. They need not share its path to briefly inherit an open
file description between fork and exec.

[Rust 1.90 File documentation source](https://github.com/rust-lang/rust/blob/1.90.0/library/std/src/fs.rs#L813)
and [Unix implementation](https://github.com/rust-lang/rust/blob/1.90.0/library/std/src/sys/fs/unix.rs#L1319)
confirm the Linux flock mapping and inherited/duplicated-descriptor lifetime.
[Linux flock(2)](https://man7.org/linux/man-pages/man2/flock.2.html) specifies that
explicit unlock releases the shared lock, whereas closing one descriptor need
not do so until all copies close. Close-on-exec does not prevent pre-exec inheritance.

A pipe-synchronized Linux fork probe, with close-on-exec set and no sleeps,
observed both cases: dropping the parent's descriptor leaves a contender blocked
while the child holds its inherited copy; explicit unlock before dropping allows
the contender to acquire while that child remains alive. Both children were reaped
and both contenders acquired after child exit. Local platform was Linux/aarch64,
kernel 7.0.0-1011-aws, glibc 2.36, distinct from the original hosted x86-64 runner.

The regression strengthens this exact Rust test: on Linux it keeps a try_clone
copy alive through resumed sync, deterministically modeling the shared open file
description. Without explicit fixture unlock, one exact selected test fails with
the same Locked error even without competing test threads. With explicit unlock,
it passes while the duplicate is still alive. While the fixture owns the lock,
the test now requires typed ErrorKind::Locked, not merely an arbitrary error.
The [machine-readable evidence](sync-lock-fixture-20261116.json) records the bounded
fork and red/green outcomes; the issue contains the original CI references.

## Classification, fix and limit

The fixture's drop-only release assumption is invalid; the confirmed mechanism
sufficiently reproduces the observed refusal. The original CI log has no FD/process
trace, so the exact inheriting child cannot be identified retrospectively. No
shared-path collision or production mutual-exclusion violation was demonstrated.
The production code correctly refuses an OS-held lock; this investigation does
not claim zero-latency release across arbitrary inherited descriptors.

Only the test fixture release changes to explicit File::unlock before drop.
Keep the Linux duplicate alive until after successful resume as the deterministic
regression; retain all platforms' existing file-preservation/nested-path checks.
There are no ignored tests, new sleeps, retries, serial-test suppression, new
dependencies or production locking changes. Full default-parallel local/CI results
are identified in the fix PR review record; no new software release is proposed.
