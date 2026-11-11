# Immutable multi-version mirrors

`assemble` combines completed frozen selections into one static distribution tree.
It works entirely from local files: resolve/download each selection first, then
assemble a new generation. Rustup remains the client. The [one-shot sync command](SYNC.md) automates these steps. Automatic scheduling,
channel resolution during assembly, retention/pruning and in-place server updates
are not implemented.

## Prepare and combine selections

The [two-version configuration](../examples/multi-version.toml) selects minimal
GNU Linux Rust 1.89.0 and 1.90.0 with the same pinned Rustup bootstrap:

```sh
rust-mirror plan --config examples/multi-version.toml --selection rust189 --output ./plan189
rust-mirror fetch --plan ./plan189 --output ./snapshot189
rust-mirror plan --config examples/multi-version.toml --selection rust190 --output ./plan190
rust-mirror fetch --plan ./plan190 --output ./snapshot190
rust-mirror assemble --snapshot ./snapshot189/mirror --snapshot ./snapshot190/mirror --output ./generation-001
```

Use `fetch --resume` for an interrupted **selection download**. `assemble` accepts
completed selected snapshots (schema 2) or earlier collection mirrors (schema 3),
not legacy schema 1. Inputs and output parents must already exist; the output
itself must be new. Stdout is the completed `OUTPUT/mirror` path. Failures return 1.

Repeat `--snapshot` for 1?32 inputs. `--previous` and `--max-bytes` are singletons;
duplicates refuse. Paths may contain spaces when quoted. No network is used by
assembly and no source snapshot is changed or removed. Outputs nested inside an
input snapshot (including the previous generation) are refused.

## Add a release or refresh a channel

```sh
rust-mirror assemble --previous ./generation-001/mirror --snapshot ./new-snapshot/mirror --output ./generation-002
```

All prior files and toolchain records carry forward unless a new input explicitly
replaces a **root** stable/beta/nightly metadata triplet. Removing a selection
from a configuration does not delete its previous release. Historical dated
manifests and archives remain available. Keep the old generation for rollback.

There is no implicit newest-version choice. Exact version inputs provide exact
version paths; they do not manufacture a `stable` alias. To refresh `stable`, first
resolve and fetch an explicit `stable` selection, review its frozen date/version,
then supply that snapshot as a new input. The supplied selection wins over the
previous generation's root alias, even if it is older; this permits deliberate
rollback. No date-based promotion or automatic backward search is performed.

Conflict rules are strict:

- Identical paths/hashes/sizes deduplicate.
- A changed archive or dated/numeric manifest at an existing immutable path refuses.
- Only `dist/channel-rust-{stable,beta,nightly}.toml` plus its `.sha256` and `.asc`
  may replace previous root metadata, as a complete coherent original triplet.
- Two new inputs disagreeing at any path refuse, including moving aliases. Re-plan
  a consistent set if a channel moved between resolutions; input order is not a
  priority rule. Use `--previous` for the old generation.
- All inputs must use the same Rustup bootstrap version. Changing bootstrap version
  is a separate migration, not silently selected by assembly.

Each retained record binds to its dated manifest, so an updated root alias cannot
rewrite the evidence for an older pinned selector. Manifests, signatures and
archives are copied unchanged. SHA checks detect consistency, not signing authority.

## Validation and storage

Before creating output, assembly verifies every inventory/file hash, validates
selected artifact closure against preserved metadata, checks record identities,
rejects unexpected files/symbolic links, and checks conflicts and final byte budget.
It then copies into private staging, hashes those copied bytes and exposes `mirror`
only after completion. An interrupted assembly may leave a reserved output directory;
use a fresh output for retry. Selection downloads remain reusable.

`--max-bytes` defaults to 10 GiB and accepts 1 byte through 100 GiB. It limits the
unique output paths' recorded bytes before copy, including retained metadata;
generated inventory/filesystem overhead is excluded. Repeated source verification
and copying read the complete inputs. There are at most 4,096 output entries,
256 retained toolchain records and a 1 MiB generated inventory. Large histories
can hit these bounds; the tool refuses instead of pruning them.

Inputs stay on disk, so budget room for the new generation too. Use trusted private
local parents; hostile concurrent mutation, shared/NFS publication and power-loss
atomicity are not qualified. Copy-time verification detects changed bytes but is
not a hostile-filesystem security boundary.

## Publish and install

The result is a **schema 3** `snapshot.toml` with retained toolchain records,
direct input inventory hashes and the combined file inventory. It is accepted by
the [same verified publisher](NEXUS-RAW.md). Publish to a new immutable prefix;
archives precede metadata and the inventory marker is last. The publisher does not
switch clients, overwrite old releases or perform a remote multi-file transaction.
Actual Nexus schema 2/3 and fixed-URL lifecycle evidence is separately tracked in
[the current record](evals/2026-11-11-rustup-lifecycle.md). The original schema 1
proof remains historical; Community Edition license onboarding is unchanged.

Point `RUSTUP_DIST_SERVER` at the generation root and install the explicit versions
or dated selectors recorded in its inventory. All selected versions share that
root and can coexist in one Rustup home. Use the [Rustup finishing stage](RUSTUP.md)
to add pinned current/archive bootstrap paths, numeric aliases and self-update
metadata. Keep automatic self-update disabled until those endpoints are verified.
Cargo crate mirroring and native SDK/linker prerequisites are not supplied.

## Offline qualification

The Linux contract-fixture proof accepts a uniquely named retained record:

```sh
sudo unshare --net python3 scripts/prove_offline.py --snapshot /absolute/generation/mirror --selection rust189 --also-selection rust190 --evidence /absolute/new-evidence
```

`--also-selection` may repeat for other uniquely named records on the tested host.
The proof installs them into the same fresh Rustup home, builds/runs a small program
with each, then selects the first toolchain again. Missing/corrupt controls still
use separate empty homes. If historical records share a selection name, this
harness refuses ambiguity; ordinary clients can select the recorded pinned selector
directly. This is not all-platform or old-system qualification.

[The dated record](evals/2026-11-10-multi-version.md) lists exact cases and limits.
