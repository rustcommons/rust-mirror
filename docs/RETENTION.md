# Local generation retention and pruning

This is the **unreleased 0.2.0 feature**, available from a source checkout. Published
0.1.0/0.1.1 binaries do not provide `prune`. Publication requires separate approval.

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

All four fields are required when the table exists. An absent table disables pruning.
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

Preview reads and hashes all inputs; it creates no files/locks and uses no network.
It validates complete inventory/selected-artifact closure and refuses corruption,
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
refuses deletion. Every apply reconstructs/recompares the plan before any unlink.

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
successful prefix in `ApplyReport::failure`. There is no cancellation/progress
callback or persistent execution-plan format in this iteration. See the compiled
[packaged API example](../src/guide.md#local-retention-unreleased).

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
size or disk quota. Private fetch/sync working copies also consume separate space.

Nexus-side pruning is deferred. The current publisher only uploads/verifies fresh
prefixes; it cannot reliably discover active Raw group membership, concurrent
clients, offline-client requirements or safe server-side deletion boundaries. This
feature sends no HTTP DELETE, changes no Nexus settings and does not unpublish a
remote copy when its local source is removed. Deleting anything on a real Nexus
requires separate owner authorization and a reviewed server-specific design.

See [requirements](REQUIREMENTS.md), [validation evidence](evals/2026-11-11-retention.md),
[command reference](CLI-REFERENCE.md#prune) and [release policy](RELEASING.md).
