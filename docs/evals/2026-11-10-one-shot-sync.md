# One-shot synchronization and demos: 2026-11-10 UTC

The owner requested one-shot synchronization and demonstrations that can be used
for testing soon. This iteration adds `sync`, runnable quick fixtures, structured
reports and a recorded real-data replay. It does not schedule a job, publish to a
server, activate client URLs, prune history or prepare a release.

## Assumptions and bounded design

[The guide](../SYNC.md) defines the command and exact counter/failure semantics.
All configured selections resolve and freeze before any payload transfer; incoherent
channel roots and immutable retained-path conflicts refuse. Resume binds canonical
config, previous inventory and saved plans. Matching payloads are reused only after
verification; unknown data still uses the official HTTPS transport. New output is
exposed only after verified generation assembly.

The existing pinned [Panamax Rustup implementation](https://github.com/panamax-rs/panamax/blob/84ca2d7064f5a9931371f9eb74c4003707897a7b/src/rustup.rs)
and [Rustup toolchain semantics](https://rust-lang.github.io/rustup/concepts/toolchains.html)
were reference context; this is not a Panamax parity claim or copied implementation.
One-shot execution is deliberately separate from scheduling and destructive retention.
The owner already authorized the local workflow; no production or EULA action follows.

## Quick fixture demonstration

[demo-sync.ps1](../../scripts/demo-sync.ps1) invokes the permanent synthetic demo;
its entrypoint was run successfully from Windows PowerShell. Cargo can obtain build
dependencies on a new checkout; the fixture itself does not download distributions.
Two synthetic selection names point to tiny test artifacts, not executable toolchains.

| Stage | Status | Metadata calls | Upstream payload bytes written |
| --- | --- | --- | --- |
| First run | created | 8 | 22 |
| Simulated connection interruption | error; no completed mirror | frozen already | partial retained |
| Saved-cohort resume | created | 0 | 5 |
| Repeat with previous mirror | unchanged | 8 | 0 |

The 22/5 byte values are **fixture-only**. The runnable demo preserves output and
reports, never overwrites an existing output and restores its process environment.
Actual child-termination tests separately verify that both sync/download OS locks
are released and that resume makes no metadata calls.

## Actual upstream metadata cases

[The 15-case sweep](one-shot-sync-metadata.json) exercised the actual sync library
with official HTTPS metadata. It includes 1.89.0/1.90.0, minimal/default, rust-src/Wasm,
GNU Linux x86_64/aarch64, Windows MSVC/GNU, Intel/Apple Silicon macOS,
stable/beta/nightly and a dated nightly, plus unavailable Miri and unknown-host
refusals. All 15 expected outcomes matched.

For 13 valid selections a frozen cohort existed before the deliberately blocked
first payload call. The other two refused before output creation. The probe fetched
metadata only and executed no installers; it is not a foreign-platform install pass
or a completed cold mirror. An initial typo in the standalone probe failed compilation;
that harness log is retained separately from its corrected build and successful sweep.

## Real incremental, unchanged and completed-resume runs

The [real demo config](../../examples/sync-demo.toml) selected minimal GNU Rust 1.89.0
and default-profile GNU Rust 1.90.0, pinned Rustup 1.28.2, and a 10 GiB budget.
It reused the prior verified two-release collection as `--previous`. That collection
already held compiler/tool archives; default-profile documentation was missing.

[Recorded invocations](one-shot-sync-runs.json) provide portable equivalent commands;
the retained private packet has exact argv and raw stdout/stderr.

| Run | Status | Metadata calls | Upstream payload written | Verified local payload written | Observed seconds |
| --- | --- | --- | --- | --- | --- |
| Incremental | created | 8 | 94,366,575 bytes | 693,938,546 bytes | 340.344 |
| Repeat | unchanged | 8 | 0 | 0 | 41.718 |
| Completed run with --resume | validated | 0 | 0 | 0 | 207.172 |

The two upstream payload calls supplied gzip/xz documentation archives. Payload
counters exclude metadata, HTTP headers and assembly I/O; these are not wire totals.
Times are observations on this machine, not performance guarantees. Some metadata
probing ran concurrently. Full hashing/copying is significant: unchanged still reads
the previous mirror, and completed resume additionally revalidates working snapshots.

The [created report](one-shot-sync-created.toml) and [validated report](one-shot-sync-validated.toml)
record both frozen versions/dates. The resulting [generation](one-shot-sync-generation.json)
has **36 recorded files plus inventory**, three retained records (including the
older distinct 1.90 selection), and **832,910,535 recorded bytes**. Every previous
file is unchanged. Inventory SHA-256:
`6db36cdfba3cb966292e028a7d69952bfd238468de87dd078a420cc7e568947d`.
No `run-002` was created for unchanged. No new Rustup install was run; original
preserved toolchain bytes retain only the preceding two-release proof's scope.
The newly mirrored documentation archives were hash-verified, not installation-qualified.

## Tests, presentation and identities

Windows passed the full suite plus final initialization/crash delta: **61 distinct
Rust tests**. Linux Rust 1.90 likewise passed **65**. Clippy, warnings-denied docs,
format checks and 12 Python tests passed; one local Windows symlink-privilege skip
remains. Final targeted tests cover killed sync processes, no writes when resuming
an unrelated empty directory, report replacement after interruption, damaged previous
and completed mirrors, workspace locks/paths, incoherent roots and storage budgets.
Existing fetch/assembly tests pass after extracting shared helpers.

The recorded replay is a self-contained HTML artifact in thread storage; it shows
captured commands/results, not a GUI product or a live server. Its three tab views
were exercised in an actual headless browser with 720px and 360px iframe viewports,
with no horizontal overflow. An initial direct headless 360px window used a wider
internal viewport; those results were not counted as mobile verification.

[Source inventory](one-shot-sync-source.json) SHA-256:
`b64bc1c5102f49374196fffe9f673a6c514e2c2bf84522f510fc265194ed9353`.
No dependencies, versions, lockfiles or licenses change. Final commit and hosted
checks are recorded in the PR.

Retained sanitized packet: `rust-mirror-sync-proof-20261110/one-shot-sync-proof.tar.gz`,
89,120 bytes, SHA-256
`801d79de788dc2b9a20683e3583958fa60e26e1eabcdf294b629fcb350950eb1`.
It includes source, bounded logs, reports, metadata cases and recorded replay;
not archived distributions, user homes, credentials/private keys or server data.

## Limits and next work

No scheduler, automatic retry loop, retention/pruning, global shared cache,
publication/activation, bootstrap migration or self-update hosting. Work and final
retained output each have a budget; overlapping selected copies count repeatedly
in work space, so even a smaller deduplicated output can exceed that budget.
Incomplete initialization may need a new output. Use private trusted local storage;
power-loss/NFS/hostile-mutation guarantees are absent. Original prototype files and
unrelated repos remain unchanged. Actual Nexus schema 2/3, Community Edition operator
onboarding, independent signatures, native foreign-platform and old-system execution
remain separate. The reports never substitute for validation or claim safe deployment.
