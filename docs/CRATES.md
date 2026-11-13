# Crate mirroring MVP contract

Owner-approved implementation scope, 2026-11-13. **Contract first; not shipped in
0.3.0.** Phase 0's exact-root research API is implemented; the workflows below are
Phase 1 development-source work until all named qualification gates pass. The
catalog, frozen plan, sync/fetch, publication and retention APIs now exist on the
implementation branch; this is not a shipped-version capability. No version or
publication is authorized. See [design](CRATES-DESIGN.md), [roadmap](ROADMAP.md)
and [requirements](REQUIREMENTS.md). Runtime dependency resolution stays native.

## Catalog and paste

A dedicated TOML catalog supports `[crates]` values in stable Cargo registry-
dependency syntax: strings, inline/expanded tables, `version`, `package`, `features`,
`default-features`, `optional` and the default crates.io registry identity.
Unknown/unstable fields, git/path (including version hybrids), rev/tag/branch,
workspace inheritance and alternate registries fail with field/alias context.
Do not read a local dependency or resolve a workspace to guess the intended source.
A library paste operation accepts a standalone `[dependencies]` block and converts
only its header; an entire project manifest is refused. Roots marked optional are
still organization-approved roots, with their declared feature profile.

```toml
[crates]
serde = { version = "1", features = ["derive"] }
rand_old = { package = "rand", version = "0.8" }
rand_current = { package = "rand", version = "0.9" }

[crate-rules.serde]
latest = 2
pins = ["1.0.228"]

[crate-policy]
features = "declared"
resolver = "2"
hosts = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"]
targets = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"]
max-final-bytes = 10737418240
max-transfer-bytes = 10737418240
max-file-bytes = 536870912

[crate-output]
base-url = "https://mirror.example/generations/004/"

[crate-network]
contact = "mirror-admin@example.org"
concurrency = 4
```

These capacity values are illustrative software limits, not organization approval
or guaranteed storage requirements. A rule is keyed by alias; default latest N=1
selects up to N highest non-yanked matching versions. Pins add exact versions even
outside the alias range and retain that alias's feature profile. No match or a
selected version missing requested features fails. Aliases express multiple ranges;
features/defaults are computed for each version. Declared feature propagation,
all-platform normal/build lock coverage and weak-forwarding `lock_only` reasons
follow the qualified native model. Dev dependencies remain excluded. Named hosts
and targets describe coverage intent; they do not prune lock metadata by platform.

`NetworkOptions` supplies explicit proxy/CA/exclusions/timeouts, including
programmatic credentials. Catalog disk fields never contain credential values.
`[crate-network]` adds contact, bounded concurrency and retry policy; it does not
implicitly opt into environment proxies. Toolchain and publisher routing defaults
remain independent. Index/CDN origins are fixed; redirects and TLS bypass remain
forbidden. An identifying User-Agent includes tool version, repository and configured
contact. Use bounded per-request retries for transient transport/429/503, bounded
Retry-After/backoff, cancellation-aware waits and a shared transfer-byte budget.
Authentication/integrity/invalid-input failures are not retried as transient errors.

## Frozen preview and one approval

`crates plan` reads index metadata, and optionally HEAD lengths; never archive GET.
It produces a versioned JSON `FrozenPlan` with a SHA-256 `plan_id`, selected roots,
original index lines, native witnesses/reasons, retained records, previous-inventory
identity, immutable output base URL, exact destination paths/counts, byte budgets
and known/unknown size estimates. Sizes are estimates; streaming/hash validation
is authoritative. No credentials, environment values or arbitrary upstream URLs
are frozen. A different observation/policy/previous/base URL changes plan identity.

Approve the entire preview once. CLI fetch/sync requires `--approve-plan ID`;
library fetch/sync takes `Approval { plan_id }`. This binds consent to the complete
selection/closure rather than a series of per-crate prompts. Approval is an explicit
caller action, not a signature or a replacement for organizational access control.
Changing the frozen plan invalidates approval. Deserialize, bound and reconstruct
plan identity/paths/metadata/closure before any transfer; a digest alone is not
trustworthy validation of attacker-supplied JSON.

`crates sync` without approval writes a new frozen preview and returns
`needs_approval`; with approval it reuses that exact saved plan, never re-resolves
between approval and transfer. `--resume` must match config/previous/frozen identity.
Changed upstream versions require a new plan/approval. Failure preserves initialized
private state; completion remains new and immutable. No implicit publish, activation
or pruning is part of sync.

## Commands and public API names

The names below are implemented on the development branch and remain subject to
all Phase 1 qualification gates before merge/release:

| CLI | Library/report |
| --- | --- |
| `crates plan --config FILE --output NEW_PLAN.json [--previous MIRROR] [--estimate-sizes]` | `crates::catalog::Catalog::parse`; `crates::mirror::plan_with_control` returns `FrozenPlan`. |
| `crates explain --plan FILE --crate NAME [--version VERSION]` | `crates::mirror::explain` returns structured root/parent/alias/requirement/features/target/lock-only chains. |
| `crates fetch --plan FILE --output NEW_RUN --approve-plan ID [--resume]` | `crates::mirror::fetch_with_control` takes a `FrozenPlan`, `Approval`, transport, output/resume options and `Control`; returns `FetchReport`. |
| `crates sync --config FILE --output RUN [--previous MIRROR] [--approve-plan ID] [--resume]` | `crates::mirror::sync_with_control` returns `SyncReport` with `needs_approval` or `completed`. |
| `publish --source MIRROR --base-url URL [--preflight]` | Existing controlled publisher accepts validated schema 4; no overwrite or automatic activation. |
| crate-aware `generations` / `prune` | Retention discovery/protection extends to schema 4; exact paths/bytes/stale-state refusal and explicit apply remain mandatory. |

`Catalog::from_dependencies` performs paste conversion and validation. Runtime
`RegistrySource` accepts explicit NetworkOptions and crate transport policy; its
index/HEAD/archive operations are separated so planning cannot accidentally fetch
payloads. Transfers use a fixed-size worker pool (default four), shared budget,
existing observer/cancel controls and structured typed errors. The GUI calls the
library; no Cargo subprocess, network/TLS reproduction or solver logic is required.

Reports/plan JSON have independent `schema` fields. Existing Phase 0 `crates::Plan`
(schema 0) is research output, never accepted as a frozen fetch plan or completed
inventory. All exported types/functions will have rustdoc. Detailed stable field
shapes are recorded with implementation and tests before claiming API availability.

## GUI field shapes and local editing

`Catalog` has a public serde/JSON representation of all five crate tables plus
optional network and `crate-retention` settings; the GUI need not parse TOML. `Catalog::to_toml` validates
and writes canonical TOML. Rewriting loses comments, original formatting and key
ordering; warn before replacing a hand-edited catalog. File replacement is explicit,
atomic within its directory, and refuses symlinks/unrelated paths.

`Catalog::from_dependencies` returns `PasteReport { catalog, accepted, rejected }`.
`accepted` lists aliases; rejected diagnostics have optional alias/field and
one-based line/column when known, a typed kind and message. Kinds include `syntax`,
`git`, `path`, `workspace`, `alternate_registry`, `unknown_field`, `invalid_value`
and `whole_manifest`. A whole manifest is refused as one typed diagnostic; the
GUI does not extract tables itself. Partial accepted values are not automatically
written or approved. A caller must explicitly choose to retain them.

Each preview version row has `name`, `version`, `cksum`, nullable `expected_bytes`,
`yanked`, `lock_only`, `root_aliases`, `primary_reason`, `retained_from_previous`
and the full structured reasons. `changes` reports added, retained,
newly-yanked-retained and no-longer-selected-retained identities. `observed_at`
is UTC Unix seconds identifying the observation; `FrozenPlan::load` performs bounded local validation
without network and returns that same saved preview. `crates show --plan FILE`
exposes the local load. Needs-approval sync results include the preview, not a
newly resolved approximation. GUI callers must show that frozen object before consent.

`explain` returns `chains`, `total_chains`, `truncated`, with a strict caller/server
chain bound; chains use the same reason objects as the preview. Budget summaries
have `exceeds_final_budget`, `exceeds_transfer_budget`, known over-size file rows
and unknown-size counts, computed by the library. Resolution errors use
`error.report` with a typed resolution kind and available alias, requirement,
feature/native-link context plus a bounded sample of candidate versions.

Concurrent operations emit `artifact_complete` for each successful artifact
operation, identifying path/action, observed bytes and whether hash verification
completed. `crate_summary` carries `files_completed`, nullable `files_total`,
transferred/reused bytes. Retry waits emit `retry` notices with path, attempt,
wait seconds, typed `kind` (`rate_limited`, `unavailable`, `transport`) and a
sanitized reason; independent heartbeats continue. A failed
artifact has no successful completion event; the final result remains authoritative.

Publish preflight refuses a destination different from the generation-local base
URL with `invalid` and both validated credential-free URLs, before any write.
`client_config` returns the exact project Cargo source-replacement TOML for a
completed generation. Schema-4 generation discovery reports `kind: "crates"` and
crate/version counts. Planning cancellation is checked between index requests and
inside native feature/search loops; blocked reads retain existing socket deadlines
and the independent heartbeat. These fields are backend work; GUI pages remain a
separately authorized phase.

## Sparse output, schema 4 and retention

A completed crate generation contains `cargo/index/config.json`, standard lowercase
sharded index files and `cargo/crates/NAME/NAME-VERSION.crate`. Generated `dl` points
to this generation's own HTTPS base URL; `api` is omitted. Every served index line
has a verified local archive. Preserve the selected upstream line bytes and original
ordering, including yank state; never reserialize records or change upstream flags.

The new root `snapshot.toml` has `schema = 4`, `kind = "crates"`, generation-local
base URL, file hash/length inventory, exact crate identities/cksums/line hashes,
root/profile/witness/reason records and previous-inventory identity. This is a crate
leaf, not an implicit new mixed toolchain format. Existing schema 1/2/3 readers must
refuse it. New publication/retention readers validate schema-4 semantic closure,
paths, schema bounds, exact hashes and symlink absence before trusting its contents.
Do not promote a schema-3-looking marker containing arbitrary crate files.

Private `.download` files, locks and frozen state stay outside the served `mirror/`.
Use OS writer locks; rehash reusable/partial files, validate range offset/length,
actual bytes and final SHA-256 against frozen `cksum`. A killed process leaves only
private resumable work; never overwrite a completed generation. Whole final and
transfer budgets apply across all workers and retained copies. Transfer bytes count
accepted archive bytes across retries/resumes, not TCP/TLS headers or discarded
socket buffers; reused copies count toward final size, not network transfer. Failed
work before the first complete managed preview/receipt may require a new workspace.
Private state is never published. Disk/controller power-loss durability is not proven.

Historical selection cohorts are stored flat, with independent native proofs, rather
than recursively embedding predecessor inventories. Current bounds are 128 cohorts,
4,096 total root profiles and 64 MiB serialized frozen metadata; exceeding a bound
refuses planning instead of silently dropping history. Retained index reordering or
immutable field changes are refused. New yank observations preserve historical bytes.

Pruning is disabled without an explicit `[crate-retention]` table. Its optional
`keep-newest` (0–32), `previous-generations` (0–31) and `pinned-generations` fields
protect whole leaves. Defaults are zero counts/empty pins; current and explicit
next-previous remain protected. Exact `crate-rules` pins additionally protect the
newest generation with that exact declared root feature/default profile and validated
closure; a missing pinned profile refuses pruning. In-progress/locked/unknown runs
are preserved/refused as in the shared [retention contract](RETENTION.md).

A successor retains previous versions and their recorded closure even when latest-N
selects newer ones. Conflicting immutable checksums/metadata fail; a new yank state
is a separate observation and does not rewrite history. New yanked selections are
refused; already verified retained yanked versions remain available for old locks.
Crate pins protect a generation containing their complete closure. Current,
next-previous, paused-run dependencies and declared rollback roots remain protected.
Pruning is explicit, previewed, revalidated and whole-generation only. No real Nexus
deletion or in-place index compaction is authorized.

Publication preserves full no-write preflight, remote absences, no overwrite,
GET/hash verification and inventory last. Prefixes are unadvertised until verified;
activation is an administrator decision. Toolchain and crate leaves can be siblings
on one static HTTPS host with separate client settings. Existing toolchain
qualification does not qualify this crate format.

## Cargo client setup

After checking an existing project config, add the same TOML on Unix and Windows:

```toml
[source.crates-io]
replace-with = "org-mirror"
[source.org-mirror]
registry = "sparse+https://mirror.example/generations/004/cargo/index/"
```

Use `cargo fetch --locked`, then `cargo build --locked` from the internal HTTPS
mirror. A fresh isolated Cargo home needs that internal network access first;
`--offline` disables it too. Test `cargo build --locked --offline` after population.
Client Cargo CA/proxy settings are independent of the connected mirroring machine.
No commands here alter the user's actual Cargo/Rustup home. Authenticated Cargo
readers, alternate/private registries and Cargo.lock import remain deferred.

## Qualification and evidence

No Phase 1 transfer/client/Nexus gate is satisfied by adopting this contract.
Required evidence, tied to exact revisions, includes:

- Approximately the top 100 most-downloaded crates, default features, with a dated
  source/ranking and every exclusion explained; retain Phase 0 tricky fixtures.
- Real Cargo HTTPS fetch/build with existing locks and isolated homes on Linux AND
  Windows; macOS when practical. Include explicit negative controls for missing or
  corrupted payloads and direct-upstream fallback.
- Resume/process termination, locks, stale-plan approval, corruption, byte/count
  budgets, yanks, links conflicts and prerelease/feature edge cases.
- Measured archive-byte overfetch against a full-index Cargo lockfile union for the
  same frozen roots/observations; distinguish metadata and unknown sizes.
- Actual disposable Nexus Raw new-prefix publication with GET verification and a
  Cargo client consuming it. Respect existing scoped image/EULA/no-production rules.

Record staged results as they occur. Open PRs with green appropriate CI; coordinator
review precedes merges. No new dependency, version bump or publication is implied.
