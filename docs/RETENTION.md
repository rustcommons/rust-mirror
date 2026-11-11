# Local generation retention and pruning

This is the **0.2.0 feature**. Published
0.1.0/0.1.1 binaries do not provide `prune`; see versioned release evidence for 0.2.0 availability.

Offline clients cannot report which toolchains they still need. Declare protection
in configuration, supply every local candidate explicitly, and inspect the preview
before authorizing deletion. Nothing prunes automatically during sync, assembly,
finishing or publication.

## Declare protection

Add this top-level table to your existing mirror configuration (after all selection
fields), or adapt [the complete example](../examples/retention.toml):

```toml
[retention]
pinned_releases = ["1.90.0", "nightly-2025-09-18"]
latest_stable = 2
nightly_days = 7
previous_generations = 1
```

The original four fields are required when the table exists. Optional
`keep_newest` (0–32, default 0) retains at least that many newest roots, while
`pinned_generations` (at most 32 unique completed root paths, default empty) pins
whole generations. Relative pinned paths resolve from the process working
directory, like CLI paths. Explicit-root ordering is declared by the caller; directory
ordering uses inventory mtime with path as a deterministic tie-breaker. Neither
ordering discovers active server routing. An absent table disables pruning.
Counts may explicitly be zero. Pins accept unique full numeric releases or dated
stable/beta/nightly selectors; moving channel names and minor aliases are refused.
Every pin must be present in at least one supplied generation or the operation
fails before deleting anything. Pins are not downloaded or resolved upstream.

Protection is the union of:

- The first supplied generation, always retained as current.
- The next `previous_generations` roots, retained for rollback.
- Every pinned release, including its distinct recorded platform/component variants.
- The numerically newest `latest_stable` distinct releases **present in the supplied
  roots**. Both numeric selections and stable-channel records count; beta/nightly
  records do not. If fewer are present, all available ones are protected.
- Nightly records in `nightly_days` calendar days including `--as-of`. For example,
  2 days with `--as-of 2025-09-18` protects September 17 and 18. Zero protects no
  nightlies. Dates after `as_of` refuse the preview; filesystem timestamps are ignored.

For each protected record, its newest equivalent copy is kept. Equivalence ignores
selection labels, but preserves selector/date/release, ordered hosts/components/
targets, profile, bootstrap version/hashes, original manifest identity and optional manager/
numeric lookup endpoint identities. Distinct
variants can retain additional generations; protection never substitutes a smaller
host/component closure. Arrays with different ordering may conservatively keep an
extra generation. Beta history needs a dated pin or rollback protection.

## Discover generations and sync runs

The GUI can call `selection::retention::discover(directory)` or use:

```sh
cargo run --locked -- generations --directory ./mirrors
```

The serializable `Catalog` lists immediate `directory/run/mirror` roots, direct
`directory/snapshot-root` folders, and managed sync/fetch workspaces. It includes
inventory identity, toolchains, recorded byte lengths and inventory mtime;
`RunInfo` distinguishes `completed`, `in_progress`, `locked` and `unknown` states.
The selected directory must be the parent containing runs/generations, not a run
workspace or private `.sync`/`.download` tree. Explicit-root pruning also refuses
roots anywhere inside those reserved working-state directories.
`unmanaged` entries are listed without recursion. `issues` identify invalid managed
paths. Discovery reads inventory/state/report metadata only; it **does not verify
payloads**, prove resume compatibility or infer which generation is active. It
creates no files or locks and only probes existing locks nonblockingly.

For a discovery-backed plan, use the exact `SyncResult::mirror` (or the finished
mirror you selected for the next run) as `next_previous`:

```sh
cargo run --locked -- prune --config mirror.toml --as-of 2025-09-22 --directory ./mirrors --next-previous ./mirrors/generation-003/mirror --dry-run
```

Use either this directory pair or explicit `--generation` arguments, not both.
The library call is `preview_directory(policy, directory, next_previous, as_of)`.
It always protects that exact root regardless of mtime/counts and protects every
matching previous-inventory identity referenced by paused sync state. Missing
anchors/dependencies refuse the plan. Paused runs, including a mirror whose sync
report has not committed, are protected. Busy/unknown runs or malformed managed
paths refuse directory pruning; discovery remains available to display them.

The plan includes `catalog`, `folders` with per-folder logical byte totals, exact
`files` and keep reasons. It validates every candidate generation and fingerprints
all managed run/generation content, including private state/cache copies, file
lengths/mtime/available creation metadata and permissions. A second scan refuses
changes during planning. Apply locks existing run writers, repeats discovery and
hashing, and refuses a changed managed file, run state or immediate directory
membership before deleting any inventory marker. Empty regular pruning-lock files
are excluded from this binding so acquiring persistent maintenance locks does not
invalidate the preview. Access times and managed-directory mtimes are excluded because
reads/maintenance lock creation can change them. Contents of unmanaged directories
and paths outside the declared directory are outside the binding and are never
traversed or deleted.

Limits are 128 immediate entries for discovery; planning accepts at most 32
completed roots, 300,000 managed entries, depth 64 and 100 GiB of managed file
content per fingerprint, plus the existing 512 MiB individual-file limit. Discovery
is metadata-only; directory planning/apply can read private copies several times
and should run off the GUI thread. Unsupported links/special files or resource
limits refuse rather than silently omit state. Stop external writers before apply;
locks cover known workspaces, not a new run created after preflight or other tools.

## Preview without deleting

Pass completed schema-2/3 snapshot roots, **newest first, current first**. This order
is an explicit administrator declaration, not inferred from path names, mtimes or
release dates. Supply `mirror` children, not entire sync/fetch working directories.
Unlisted paths are never discovered or deleted. A generation is self-contained;
its `inputs` inventory hashes do not require keeping the original input directories.

```sh
cargo run --locked -- prune --config mirror.toml --as-of 2025-09-22 --generation ./generation-003/mirror --generation ./generation-002/mirror --generation ./generation-001/mirror --dry-run
```

Omitting both `--dry-run` and `--apply` also previews. Stdout is TOML with the exact
`generations` and `files` to delete, each file's absolute `path`, logical `bytes`
and SHA-256, `total_bytes`, and `kept` roots with reasons. It also includes the frozen
policy, root order and inventory identities. Snapshot inventory bytes are counted;
empty directory removals are implied by the listed roots/files. Sizes are logical
lengths, not physical reclaimed disk blocks (hardlinks, compression and open handles
can change actual savings).

Preview reads and hashes all inputs; it creates no files/lock files and uses no
network. Existing writer locks are probed transiently without blocking.
It validates complete inventory/selected-artifact closure and refuses inputs belonging
to locked, unknown or incomplete sync runs, as well as corruption,
legacy schema 1, unrelated files or empty directories, duplicate/overlapping roots,
parent traversal, non-UTF-8 root paths and symbolic links in roots, ancestors or payload trees.
Limits: 1–32 roots, 256 pins/latest-stable count, 36,600 nightly days, 31 rollback
roots, plus existing snapshot inventory/file/record/individual-file bounds. Missing
roots are errors; there is no implicit skip, force or corruption bypass.

## Apply explicitly

Stop serving/using deletion candidates and prevent external writes. Confirm that
the first root really is active/current, that offline-client protection is complete
and that rollback candidates are ordered correctly. Then repeat the same command
with `--apply` instead of `--dry-run`:

```sh
cargo run --locked -- prune --config mirror.toml --as-of 2025-09-22 --generation ./generation-003/mirror --generation ./generation-002/mirror --generation ./generation-001/mirror --apply
```

The CLI computes a fresh preview and applies that in-memory plan. It does not read
a saved TOML preview as execution authority. For confirmation across a GUI interaction,
retain the `PrunePlan` in memory and call the library `apply` on that same object.
The GUI must present it and require explicit user authorization; passing `false`
refuses deletion. `PrunePlan` supports Serde serialization/deserialization between
identical library builds, including a GUI's embedding worker and parent. Schema 1
is a retention-plan revision, not an EVENTS protocol version. Apply checks envelope
bounds and reconstructs all plan fields before deletion; deserialized data is never
trusted as authorization. Callers must bound transport/deserialization input size. Every apply reconstructs/recompares the plan before any unlink.

Apply acquires persistent empty `.rust-mirror-prune.lock` files in all supplied
roots' enclosing workspaces, plus any existing `.sync.lock`/`.fetch.lock`. Busy or
malformed locks refuse preflight. Lock files and enclosing workspaces are preserved.
These locks coordinate pruning and existing sync/fetch writers only; they do not
coordinate HTTP readers, publishers, unrelated programs or hostile shared filesystems.
Use trusted private local parents and exclusive maintenance; no NFS/shared-machine
or hostile-filesystem safety guarantee is claimed.

Only listed files are unlinked, then only their empty directories inside approved
roots. The inventory marker is removed first so interrupted trees fail validation.
There is no recursive deletion of workspaces, private `.sync`/`.download` state or
unlisted content. Files are checked again before unlinking. Apply returns a TOML
report containing `deleted_files`, `deleted_bytes`, `removed_generations` and an
optional `failure` with path/message. Any failure stops processing, leaves successful
deletions in effect and makes the CLI return exit code 1. Preflight failures delete
nothing, although persistent lock files may have been created. Process termination
can lose the report; deletion is not a transaction or recoverable backup.

After an I/O failure/termination, inspect the report and remaining roots manually.
Do not publish partial roots or blindly retry the original list: missing/partial
inventories refuse the next preview. Restore from a separately retained generation
or explicitly repair/remove the already-approved partial root after inspection.

## Library and GUI

`selection::retention::{preview, apply}` returns structured, serializable
`PrunePlan`/`ApplyReport` values, without printing progress. Calls are synchronous
and can hash many files; GUI callers should run them off their UI thread. Errors
before deletion use the common `Result`; post-preflight I/O failures carry the
successful prefix in `ApplyReport::failure`. Controlled preview/apply variants accept `&events::Control` and emit progress/hash/
heartbeat events with cooperative cancellation. Cancelled apply errors can contain
successful-prefix evidence in `Error::report`; process termination can still lose it.
The CLI uses the [shared protocol](EVENTS.md). Same-build Serde round trips are supported; no durable cross-version saved-plan
compatibility is promised. Serialized previews do not grant deletion authority. See the compiled
[packaged API example](../src/guide.md#local-retention).

Adding optional `Config::retention` preserves existing TOML without the table.
Rust callers constructing `Config` literals must add `retention: None` or an explicit
policy. Changing the config, including retention, can invalidate a frozen sync
resume identity; finish/reuse or explicitly restart the run under its existing rules.
The package version remains 0.1.1 until separate release preparation.

## Granularity and Nexus gap

Pruning removes **whole local snapshot roots**. It never edits or compacts files or
records inside a kept immutable generation. If one protected record retains a root,
all its other releases are retained too. Additive collections can therefore exceed
requested release counts; those counts are minimum protection, not an exact cache
size or disk quota. Private fetch/sync working copies also consume separate space. Discovery lists them,
but whole-workspace deletion is deferred: state, plans, reports, caches and incomplete
runs are retained intact. Apply may acquire existing writer locks for inspection;
it never resets, edits or deletes paused state. An incomplete run with a missing
previous root must be restored/repaired explicitly before directory pruning.

Nexus-side pruning is deferred. The current publisher only uploads/verifies fresh
prefixes; it cannot reliably discover active Raw group membership, concurrent
clients, offline-client requirements or safe server-side deletion boundaries. This
feature sends no HTTP DELETE, changes no Nexus settings and does not unpublish a
remote copy when its local source is removed. Deleting anything on a real Nexus
requires separate owner authorization and a reviewed server-specific design.

See [requirements](REQUIREMENTS.md), [validation evidence](evals/2026-11-11-retention.md),
[command reference](CLI-REFERENCE.md#prune) and [release policy](RELEASING.md).
