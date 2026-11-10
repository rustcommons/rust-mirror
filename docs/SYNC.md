# One-shot synchronization and demos

`sync` automates **resolve -> freeze -> fetch/reuse -> verify -> assemble** for all
named selections in one configuration. It runs once and exits. It does not publish,
activate a Nexus URL, schedule itself, prune history or install toolchains.

## Try the quick demo first

From a checkout with Rust/Cargo 1.90+:

```powershell
.\scripts\demo-sync.ps1
```

It runs in about a second after compilation and preserves its output location.
It uses tiny synthetic archives: **no Rust distribution download or installation**.
You see first-run creation, a simulated connection interruption, a resume using
saved metadata, and an unchanged repeat run. Reports are left under the printed
path. The script never deletes an existing output or changes a user's toolchain home.
Cargo may need to obtain the project's build dependencies on a fresh checkout.

Equivalent portable fixture command:

```sh
RUST_MIRROR_DEMO_ROOT=/absolute/new-demo cargo test --locked --test sync demo_tiny_sync -- --exact --nocapture
```

Do not mistake these tiny fixture byte counts for real archive sizes. The
[recorded real-data demo](evals/2026-11-10-one-shot-sync.md) uses actual distributions.

## Build and run the real CLI

```sh
cargo build --locked
# Use target/debug/rust-mirror, or target/debug/rust-mirror.exe on Windows.
rust-mirror sync --config examples/sync-demo.toml --output ./run-001
```

The [demo config](../examples/sync-demo.toml) selects GNU Linux Rust 1.89.0 minimal
and 1.90.0 default. A first population without reusable data downloads hundreds of
megabytes; archive sizes are not known until transfer. It uses the existing 10 GiB
byte budget. Choose versions, hosts, components and targets before running.
A host is where the compiler will run, not necessarily where the mirror CLI runs.
This example mirrors Linux distributions even when invoked from Windows.

Use [discovery/import](SELECTIONS.md) to create your own saved selections. Source
URLs remain official verified HTTPS; original manifests/signatures are preserved.
Independent signature verification and native SDK provisioning remain separate.

## Incremental and unchanged runs

```sh
rust-mirror sync --config examples/sync-demo.toml --previous ./run-001/mirror --output ./run-002
```

The previous mirror must be a complete selected (schema 2) or collection (schema 3)
snapshot. Its files, metadata and artifact closure are revalidated. Matching payload
bytes can be reused by SHA-256 from that mirror or an earlier selection in this run.
Missing bytes use the upstream transport. A damaged previous mirror is refused,
not silently repaired. Previous inputs are never edited.

If every requested record and file is already present, the result is **unchanged**:
stdout points to the previous mirror and `run-002` is not created. No payload
transfer occurs, but current metadata is still checked. All prior files are hashed,
so a large unchanged mirror still needs disk I/O. Extra retained releases stay;
omitting a selection does not delete it.

For a changed result, `OUTPUT/mirror` is the new immutable generation. Existing
outputs refuse unless explicitly resumed. Output parents must exist and cannot be
inside the previous mirror. Publish separately using the [hosting guide](NEXUS-RAW.md).

## Resume an interrupted run

```sh
rust-mirror sync --config examples/sync-demo.toml --previous ./run-001/mirror --output ./run-002 --resume
```

Use the same semantic configuration and previous inventory identity. The whole
cohort's metadata was frozen **before** payload downloads; resume uses those saved
plans with zero live metadata resolution. It finishes that historical cohort even
if stable/nightly has changed since. Start a fresh output for a newly resolved run.
Changed config, budget, previous identity, plans or unexpected workspace paths refuse.

`.sync` holds private plans, selected snapshots and assembly progress behind an OS
single-writer lock. Do not serve it. After a killed process the OS releases both
sync and selection locks. Initial setup interrupted before its identity is complete
may require a new output. Staging copy work can be rebuilt; payload progress remains
available. Retained selected copies currently stay for revalidation/resume and cost
additional disk space. There is no shared global cache or pruning.

## Results and counter meaning

Stdout is only the mirror path; stderr shows stages and a final summary:

```text
SYNC created | ... selections | ... files | ... recorded bytes
  name: requested -> pinned selector (resolved release)
  metadata requests: ... | upstream payload bytes: ... | local payload bytes: ...
```

Status is `created`, `unchanged`, or `validated` for an already completed resumed
run. Success, including unchanged, returns 0; errors return 1. A created/resumed
workspace stores the latest completed invocation in `.sync/report.toml`, replaced
atomically. The report is not a freshness certificate: all data is revalidated.
Unchanged runs do not create an output/report; their summary is on stderr.

`metadata_requests` counts metadata transport calls, and `upstream_attempts` counts
payload transport calls. Payload byte counters measure bytes written through that
source into work files, including prefixes retained after an interruption. They
exclude HTTP headers, metadata bytes, assembly I/O and already verified workspace
files reused directly by the download cache. They are **not wire-traffic totals**.
`local_payload_bytes` includes verified reuse from previous/earlier selected files.
All numbers describe the current invocation; interrupted invocations do not have a
completed report. File/byte totals exclude the generated inventory itself.

## Limits and coherence

The configuration byte budget applies separately to the combined selected working
copies and the final generation including retained files. Thus overlapping selected
copies can hit the working budget even if their deduplicated generation would fit.
Temporary staging/cache space can approach twice that budget, plus metadata,
inventories and the separate previous generation. It is not an OS disk quota.
At most 32 selections, 4,096 final files, 256 records and a 1 MiB final inventory;
existing per-file/metadata limits still apply. Unknown new sizes can cause a later
budget refusal; no completed output is exposed on verification failure.

Conflicting selections/channel resolutions are detected before payload download.
Only explicit coherent root stable/beta/nightly triplets may replace previous
aliases; immutable path conflicts and bootstrap-version changes refuse. No automatic
fallback to a different nightly or silent omission of requested components.

Private trusted local parents and working filesystem locks/renames are required.
Power-loss durability, hostile mutation and NFS/multi-machine locking are unqualified.
Publication, scheduling, retention/deletion, self-update hosting and current Nexus
schema 2/3 onboarding are not implemented or newly qualified by this command.
