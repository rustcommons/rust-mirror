# Command and configuration reference

For the complete first-run/update walkthrough, read the [user guide](USER-GUIDE.md).
This reference describes the current source CLI, including the 0.2.0
retention and events additions. Published 0.1.x binaries do not include these features. `rust-mirror`
prepares/hosts files; the separate `rustup` application installs client tools.

## Invocation and common rules

```text
rust-mirror --help
rust-mirror --version
rust-mirror COMMAND --option VALUE ...
```

- `--help` / `-h` print top-level usage; no command also prints usage. `--version`
  / `-V` print this program's version. These are top-level forms; subcommand
  `--help` is not implemented except for the namespace-level `crates --help`. Global help/version return immediately; trailing
  arguments are not validated by those forms.
- Supply values as separate arguments (`--output DIR`), not `--output=DIR`.
  Quote paths/names with spaces. There are no positional path arguments or implicit
  config/environment selection overrides.
- Required means the CLI has no default. Unknown command/options, missing values
  and repeated singleton options fail. Only `assemble --snapshot` and
  `rustup --promote` and `prune --generation` are repeatable options, within their documented limits.
- Success returns 0; errors return 1, including network/validation/refusal errors.
  There are no distinct HTTP or checksum exit codes. Diagnostics use stderr.
- New output paths need existing parents. No implicit overwrite is offered.
  Resume/force exceptions are explicit below. Completed snapshots are immutable.
- Remote publication, local preparation and Rustup client settings are independent.
  Neither `sync` nor `rustup` automatically uploads or activates anything.

## Global events and cancellation

`--events jsonl` is an opt-in global flag. Stdout becomes protocol-1 JSON lines;
human diagnostics stay on stderr. `--events jsonl --version` is cheap local backend/
protocol discovery. Add `--cancel-stdin` to accept a literal `{"type":"cancel"}`
line or EOF as an idempotent stop request; otherwise stdin is not a control channel.
Ctrl-C and supported process-stop signals request cooperative cancellation too.
Independent 1-second heartbeats include stage, operation, current path, last progress
sequence and pending cancellation. The GUI owns watchdog/grace/kill behavior and
must drain both output pipes. Exit codes are 0/1/130 for success/failure/cancel.

See [protocol 1](EVENTS.md) for fields, nullable sizes, typed result kinds, sequencing,
limits of blocked reads and command-specific recovery. Fetch/sync initialized state
supports explicit resume; assemble/rustup require a new output after interruption;
publish/prune may have partial effects. No global events flag changes deletion or
overwrite authority.

## Command overview

| Command | Purpose | Network | Result on stdout |
| --- | --- | --- | --- |
| [`init`](#init) | Write example configuration | None | Config file path |
| [`list`](#list) | Inspect upstream release availability | Metadata only | TOML catalog |
| [`import`](#import) | Convert a project toolchain declaration into a mirror config | None | New config path |
| [`plan`](#plan) | Freeze one selection's metadata and expected artifacts | Metadata/checksums only | TOML plan |
| [`fetch`](#fetch) | Download/verify a frozen plan | Planned payloads only | Completed mirror path |
| [`sync`](#sync) | Resolve/fetch/assemble all selections once | Metadata and missing payloads | New or previous mirror path |
| [`assemble`](#assemble) | Combine completed local snapshots/history | None | Completed mirror path |
| [`rustup`](#rustup) | Add bootstrap/self-update endpoints and explicit promotion | Rustup script/checksums/missing installers | Completed mirror path |
| [`generations`](#generations) | List local generations and managed runs | None, metadata reads only | TOML catalog |
| [`prune`](#prune) | Preview/explicitly delete whole local generations | None | TOML preview or apply report |
| [`publish`](#publish) | Upload/GET-verify a fresh remote prefix | HEAD/PUT/GET to selected endpoint | TOML publication report |
| [`fetch-toolchain`](#fetch-toolchain) | Legacy pinned minimal GNU Linux snapshot | Metadata/payloads | Completed mirror path |

### init

```sh
rust-mirror init --config mirror.toml
```

| Option | Required | Meaning |
| --- | --- | --- |
| `--config FILE` | Yes | New config file populated from the packaged example |

The example includes multiple selections. Edit/remove unwanted ones before sync.
No OS inference or download occurs. Existing files are refused.

### list

```sh
rust-mirror list --toolchain 1.90.0 --host x86_64-pc-windows-msvc
```

| Option | Required | Meaning |
| --- | --- | --- |
| `--toolchain SELECTOR` | Yes | Full numeric release or supported channel/date |
| `--host HOST` | No | Include that host's components, aliases and compilation targets |

Without a host, output describes release hosts/profiles. With one, output includes
its tooling availability. Consult `supported_profiles`, not just all upstream
`profiles`. No payload archives are fetched. Availability does not prove native
execution, bootstrap availability or that a later download will succeed.

### import

```sh
rust-mirror import --toolchain-file rust-toolchain.toml --name project --hosts x86_64-pc-windows-msvc --rustup-version 1.28.2 --output project-mirror.toml
```

| Option | Required | Meaning |
| --- | --- | --- |
| `--toolchain-file FILE` | Yes | Standard `[toolchain]` TOML or legacy ASCII selector file |
| `--name NAME` | Yes | Name for the generated mirror selection |
| `--hosts HOST[,HOST]` | Yes | Comma-separated explicit hosts with no surrounding spaces; no implicit current host |
| `--rustup-version VERSION` | Yes | Bootstrap manager pin, not compiler version |
| `--output FILE` | Yes | New mirror config file |

Imports channel/profile/components/targets, validates supported selectors, and
refuses local paths/unknown fields. No parent-directory search or network access.
See [import details](SELECTIONS.md#import-a-projects-toolchain-file).

### plan

```sh
rust-mirror plan --config mirror.toml --selection windows-team --output plan-windows
```

| Option | Required | Meaning |
| --- | --- | --- |
| `--config FILE` | Yes | Saved validated mirror configuration |
| `--selection NAME` | Yes | Exactly one named selection in that file |
| `--output DIR` | Yes | New directory holding plan and cached metadata |

Fetches original manifest/checksum/signature metadata and bootstrap checksum
metadata, not compiler/install archives. It prints TOML, saves the frozen plan and
reports that archive sizes remain unknown until download. Changing stable later
does not change this saved plan. Do not edit generated metadata or hashes.

### fetch

```sh
rust-mirror fetch --plan plan-windows --output snapshot-windows
rust-mirror fetch --plan plan-windows --output snapshot-windows --resume
```

| Option | Required | Meaning |
| --- | --- | --- |
| `--plan DIR` | Yes | Completed frozen plan directory, not just its TOML file |
| `--output DIR` | Yes | New download workspace, or the same initialized resumable workspace |
| `--resume` | No | Validate/reuse saved progress for this exact plan/output |

Produces `DIR/mirror` after hashes/closure checks. All advertised archive formats
of selected components are included. Existing output without resume refuses.
Range continuation validates offsets/lengths and rehashes final bytes; servers
ignoring ranges cause a safe whole-file restart. There is no automatic retry or
fetch force option. Completed resumed output is validated, never overwritten.
Uninitialized/corrupt/foreign progress may need a new path. See [resume rules](RESUMING.md).

### sync

```sh
rust-mirror sync --config mirror.toml --output run-001
rust-mirror sync --config mirror.toml --previous run-001/mirror --output run-002
rust-mirror sync --config mirror.toml --previous run-001/mirror --output run-002 --resume
rust-mirror sync --config mirror.toml --previous run-001/mirror --output run-002 --force
```

| Option | Required | Meaning |
| --- | --- | --- |
| `--config FILE` | Yes | Every named selection is processed |
| `--output DIR` | Yes | Local managed run/workspace directory |
| `--previous MIRROR` | No | Completed local schema 2/3 mirror to retain/reuse, not a run root or URL |
| `--resume` | No | Continue the identical frozen config/previous/output cohort |
| `--force` | No | Destructively discard recognized managed old progress/mirror and resolve afresh |

`--resume` and `--force` are mutually exclusive. Without either, an existing output
refuses before network access. Force has **no backup or rollback**, cannot bypass
an active writer, and refuses unrelated/link/overlapping paths. Old output is
removed before fresh resolution can fail. An external previous mirror is preserved. On a new output path force behaves like
ordinary sync, including unchanged detection; an existing managed run is rebuilt
even when previous data can satisfy its payloads.

All cohort metadata freezes before payload transfer. Resume performs no live
metadata resolution. Previous files are validated/hashed; matching payloads can be
reused. Omitting an old selection does not prune its retained files. Incompatible
bootstrap versions and conflicting release metadata refuse instead of guessing.

An unchanged new run prints the **previous** mirror path without creating output.
A new result is `DIR/mirror`; a completed resumed result is validated. Use the
returned path, not an assumed `--output/mirror`. No automatic scheduling,
publication, activation or pruning. [Counters/recovery](SYNC.md).

### assemble

```sh
rust-mirror assemble --snapshot snapshot-a/mirror --snapshot snapshot-b/mirror --previous generation-old/mirror --output generation-new
```

| Option | Required | Meaning |
| --- | --- | --- |
| `--snapshot MIRROR` | Yes, 1-32 | Repeat for new completed schema 2/3 inputs |
| `--previous MIRROR` | No | One completed retained schema 2/3 generation |
| `--output DIR` | Yes | New output directory; result is `DIR/mirror` |
| `--max-bytes INTEGER` | No | Final recorded file-byte budget, default 10 GiB, maximum 100 GiB |

Entirely local; no new release resolution or input editing. Inputs need compatible
top-level Rustup pins. Immutable conflicts and ambiguous incoming channel roots
refuse. Only explicit coherent incoming channel roots replace retained roots;
assembly never chooses an implicit winner. No resume/force or schema 1 input.
[Conflict and storage rules](MULTI-VERSION.md).

### rustup

```sh
rust-mirror rustup --snapshot run-002/mirror --version 1.29.3 --promote stable-2025-09-18 --output complete-002
```

| Option | Required | Meaning |
| --- | --- | --- |
| `--snapshot MIRROR` | Yes | Completed schema 2/3 input, not legacy schema 1 |
| `--version VERSION` | Yes | Chosen Rustup manager version advertised by the generated catalog |
| `--output DIR` | Yes | New output; completed result is `DIR/mirror` |
| `--promote CHANNEL-DATE` | No; repeat within limits | One explicit dated stable/beta/nightly selector per channel, already present as a matching verified record |
| `--max-bytes INTEGER` | No | Completed recorded-byte budget, default 10 GiB, maximum 100 GiB |

Copies retained files, adds verified archived/current installers/checksums for
recorded hosts and the original shell installer. Generates the pinned self-update
catalog and full/minor stable numeric lookup policy. Original Rust metadata stays
unchanged. A minor alias selects the highest retained selected stable patch,
not an unmirrored public patch. Dated promotion copies a coherent original triplet.

This `--version` is not top-level `rust-mirror --version` and not compiler version.
Manager/script/checksum decisions freeze before installer payload transfer.
An unchanged immutable archive identity may be reused; a changed one refuses.
No resume/force, server write or client update. See [lifecycle details](RUSTUP.md).

### publish

```sh
rust-mirror publish --source complete-002/mirror --base-url https://nexus.example.local/repository/rust-generation-002 --ca-file /path/to/company-ca.pem
```

| Option | Required | Meaning |
| --- | --- | --- |
| `--source MIRROR` | Yes | Complete validated schema 1/2/3 static tree |
| `--base-url HTTPS_URL` | Yes | Fresh remote prefix receiving `dist/`, `rustup/`, inventory beneath it |
| `--preflight` | No | Validate local closure/TLS/auth responses and HEAD-absence of all remote paths; no PUT/DELETE, not a reservation or proof of upload permission |
| `--ca-file PEM` | No | Explicit CA bundle for this publisher connection instead of default roots |
| `--allow-loopback-http` | No | Test-only literal `127.0.0.1`/`[::1]` HTTP; never with credentials or CA option |

Set both publishing credential variables or neither. URL userinfo, query,
fragments/encoded paths, redirects and automatic proxy discovery are refused/not
used. The native CLI's default TLS roots are not a promise to use the OS trust
store; supply the required private PEM bundle explicitly.

All local validation and all remote absence checks precede the first PUT. Each
uploaded file is GET-hashed; the inventory marker is last. Success does not switch
routing or prove your server's policy. Require an exclusive publisher and server
no-redeploy enforcement. On failure, an incomplete prefix can remain. No remote
force, resume, repair, retry, repository creation or license/settings change.
Per-request deadline is 180 seconds. [Nexus behavior](NEXUS-RAW.md).

### fetch-toolchain

```sh
rust-mirror fetch-toolchain --release 1.90.0 --host x86_64-unknown-linux-gnu --rustup-version 1.28.2 --output legacy-snapshot
```

All four shown options are required: `--release` is the compiler's full numeric
version, `--host` is **only** `x86_64-unknown-linux-gnu`, `--rustup-version` pins its
bootstrap and `--output` is a fresh directory. This legacy schema 1 minimal-profile
route selects a preferred archive format rather than all advertised formats and
has no resume/force or full self-update finishing. Use selections/sync/rustup for
the general 0.1.0 workflow. See [the historical example](NEXUS-RAW.md#prepare-on-the-connected-machine).

There is no standalone `serve`, `verify`, `doctor` or `schedule` command in 0.1.0.
Generation creation and publication validate their inputs; `fetch --resume`
revalidates an already completed matching workspace. This is not a general-purpose
verification command for arbitrary files.

### generations

```sh
cargo run --locked -- generations --directory ./mirrors
```

`--directory DIRECTORY` is required and the only option. Read-only metadata discovery
lists direct snapshot folders and `run/mirror` roots plus completed/in-progress/locked/
unknown sync/fetch runs, inventory identities/recorded sizes, unmanaged entries and
classification issues. It does not verify payloads or promise resume compatibility.
At most 128 immediate entries are inspected; no files/locks/network requests are created.

### prune

Available in 0.2.0:

```sh
cargo run --locked -- prune --config mirror.toml --as-of 2025-09-22 --generation ./generation-002/mirror --generation ./generation-001/mirror --dry-run
```

| Option | Required | Meaning |
| --- | --- | --- |
| `--config FILE` | Yes | Existing strict mirror config with explicit `[retention]` policy |
| `--as-of YYYY-MM-DD` | Yes | Calendar reference date; no implicit clock/mtime policy |
| `--generation MIRROR` | Explicit-root mode, repeatable | 1–32 completed schema-2/3 roots in declared newest-first order; first is current and always kept |
| `--directory DIR` | Alternative | Discover candidates instead of passing explicit roots; requires `--next-previous` |
| `--next-previous MIRROR` | With directory | Exact completed root needed for the next sync; always protected |
| `--dry-run` | No | Read-only exact preview; also the default when neither mode flag is given |
| `--apply` | No | Explicitly delete approved whole snapshot roots after locks and revalidation; mutually exclusive with dry-run |

Use either explicit generation roots or the directory/next-previous pair. Directory
mode protects paused-run dependencies and binds the managed directory state to apply.
Busy/unknown runs refuse pruning. Whole private-workspace deletion is deferred.

No network or remote deletion occurs. Preview returns per-folder `folders` sizes, exact file `path`, `bytes`,
`sha256`, `total_bytes`, kept roots/reasons and frozen inputs/policy. Apply returns
`deleted_files`, `deleted_bytes`, `removed_generations` and optional `failure`.
A partial failure returns exit 1 and leaves earlier deletions applied. Read
[retention and recovery](RETENTION.md) before apply: enclosing workspaces and kept
collections are preserved; locks are advisory, not live-reader protection.

The optional top-level `[retention]` table requires all four fields:

| Field | Required within table | Meaning/bounds |
| --- | --- | --- |
| `pinned_releases` | Yes | 0–256 unique full numeric or dated-channel selectors; missing pins refuse |
| `latest_stable` | Yes | 0–256 newest distinct numeric stable releases present in the supplied roots |
| `nightly_days` | Yes | 0–36,600 calendar days including as-of; zero disables this protection |
| `previous_generations` | Yes | 0–31 roots immediately after current kept for rollback |
| `keep_newest` | No, default 0 | 0–32 newest roots; minimum protection in addition to current and other rules |
| `pinned_generations` | No, default empty | Up to 32 unique completed root paths that must be present |

Absent table disables pruning. Protection is additive and keeps every distinct
recorded platform/component/manifest/bootstrap variant in its newest matching root.
Counts are minimum protection, not a disk quota; a kept root retains all its files.
`sync` never invokes retention. Existing config TOML is compatible, but `Config`
struct literals need `retention: None` (or a policy), and config changes affect sync
resume identity. See [configuration example](../examples/retention.toml).

## Configuration fields

Configuration is strict TOML schema 1. Unknown fields are errors, not ignored hints.
See the [walkthrough's complete configurations](USER-GUIDE.md#worked-example-first-generation)
and [packaged multi-platform example](../examples/mirror.toml).

| Field | Required/default | Meaning |
| --- | --- | --- |
| `schema` | Required: `1` | Mirror configuration format |
| `rustup_version` | Required | Full numeric bootstrap manager pin used during plan/fetch/sync |
| `max_download_bytes` | Default `10737418240` | 10 GiB recorded-byte budget; range 1 through 100 GiB |
| `[[selections]]` | Required, 1-32 unique names | All are processed by sync; plan names one |
| `name` | Required | Unique human selection name, nonblank, at most 80 bytes, no controls |
| `toolchain` | Required | Full numeric version or supported channel/date, not partial versions/local paths |
| `hosts` | Required, 1-16 unique | Compiler execution platforms, not auto-detected |
| `profile` | Default `default` | `minimal` or `default`; upstream release determines component list |
| `components` | Default empty, at most 64 unique | Extra release-specific tools/aliases such as `clippy`, `rustfmt`, `rust-src` |
| `targets` | Default empty, at most 64 unique | Extra compilation-target standard libraries |

Compiler/manager versions are distinct. A later finishing version can differ from
the raw bootstrap pin, but combining raw inputs requires compatible top-level pins.
All hosts in a selection share its requests. Explicit unavailable requests fail;
there is no quiet shrink, newest-older-nightly fallback or independently versioned
Cargo/Clippy mix. Identifier, calendar-date and official-origin/path checks apply.

## Environment variables and trust

| Variable/setting | Consumed by | Meaning |
| --- | --- | --- |
| `NEXUS_USERNAME`, `NEXUS_PASSWORD` | rust-mirror CLI `publish` | Both UTF-8 nonempty values for HTTPS Basic auth, or neither; not stored in the inventory |
| `RUSTUP_DIST_SERVER` | Rustup client | Complete generation/group root serving `/dist/...` |
| `RUSTUP_UPDATE_ROOT` | Rustup client | That root plus `/rustup`, serving manager catalogs/installers |
| `CARGO_HOME`, `RUSTUP_HOME` | Cargo/Rustup client | Client settings/download/toolchain locations; use separate homes for tests |
| OS trust / `SSL_CERT_FILE` / `CURL_CA_BUNDLE` | Their respective client backends | Not interchangeable with native publisher `--ca-file`; Windows/macOS Rustup need appropriate platform trust |
| `RUST_MIRROR_DEMO_ROOT` | Synthetic Rust test/demo only | New output location for the fixture demo, not production CLI configuration |

The library publisher takes credentials explicitly in `publish::Options`; it does
not read those credential variables. The native publisher explicitly disables
proxy auto-discovery. The locked CLI's upstream downloader also does not enable
ureq's optional environment-proxy feature. There is no CLI proxy option or upstream
CA override; arrange verified HTTPS access to the official source. A library
application can supply its own `Source` transport and must own its trust contract.
No command supports a skip-TLS-verification option.

Rustup's environment settings do not alter rust-mirror selections or make it install
client tools. Conversely, setting publisher credentials/CA does not configure
Rustup client authentication or OS trust. Do not put secrets in URLs, command
history, diagnostics, configuration examples or transferred inventories.

## Output and report fields

A path on stdout is the returned result, not always `--output/mirror`: an unchanged
sync returns the previous tree. `list`/`plan`/`publish` output TOML. Core diagnostics
and transfers go to stderr; capture it separately if parsing stdout.

### Discovery catalog

| Field | Meaning |
| --- | --- |
| `toolchain`, `release`, `date` | Requested selector, resolved compiler-version text and manifest date |
| `hosts` | Records with `name` and upstream `available` flag |
| `profiles`, `supported_profiles` | Upstream names versus the subset accepted by this implementation |
| `components` | Host/global component records: `package`, `aliases`, `target`, `available`, `manifest_package_version` |
| `targets` | Compilation-target records with `name` and `available` |

Components/targets are populated for an explicit host. A component's version text
is whatever its manifest package records, not an independently selectable tool pin.
`target = "*"` means a global artifact such as sources. Upstream can reuse one
architecture-named documentation archive for another host; follow its manifest URL,
not an assumption based on the filename.

### Frozen plan

| Field | Meaning |
| --- | --- |
| `schema`, `planner_revision` | Plan-format and planner-compatibility versions, not Rust/Rustup versions |
| `selection` | Validated request, with the same fields as the saved selection |
| `rustup_version`, `max_download_bytes` | Frozen bootstrap pin and byte limit |
| `resolved_release`, `date`, `pinned_toolchain` | Actual compiler, metadata date and stable numeric/dated client selector |
| `metadata_source` | Relative official manifest source path |
| `manifest_sha256` | Hash of saved original manifest bytes |
| `checksum_sha256`, `signature_sha256` | Hashes of saved sidecar/signature file bytes, not proof the signature was verified |
| `bootstrap_checksums` | Host-to-hash map of saved installer checksum-sidecar bytes |
| `artifacts` | Records with relative `path`, expected payload `sha256`, `package`, `target`, and `kind` (`component` or `bootstrap`) |

Artifact sizes are not guessed into the plan. Replaying validates saved metadata,
its hashes and derived artifact closure; editing TOML is not a supported way to
change a plan. Save a newly resolved plan instead.

### Snapshot inventories

All `files` entries contain a relative `path`, SHA-256 `sha256` and recorded `bytes`.
The inventory does not list itself as one of its own files.

| Format | Other fields |
| --- | --- |
| Legacy schema 1 | `release`, `host`, `rustup_version`, fixed `profile`, `files` |
| Selected schema 2 | `release`, `date`, `pinned_toolchain`, `rustup_version`, `selection`, `files` |
| Collection schema 3 | `rustup_version`, `toolchains`, direct `inputs` inventory hashes, `files` |

Each collection toolchain record has `selection`, `release`, `date`, and
`pinned_toolchain`. `inputs` identifies directly supplied inventories, not a signed
provenance chain or a promise that old input directories must remain present for
serving the new self-contained generation. Clients use the static `dist`/`rustup`
paths; the inventory is for administrative verification, not Rustup's manifest.

### Sync report

A completed sync writes `.sync/report.toml` for the latest completed invocation.
An unchanged new run creates neither the new output nor a new report. Fields:

| Field | Meaning |
| --- | --- |
| `status` | `created`, `unchanged`, or completed-resume `validated` |
| `selections` | Requested selection count |
| `frozen` | `name`, `requested`, `pinned_toolchain`, `release`, `date` per selection |
| `metadata_requests` | Metadata transport calls during this invocation |
| `upstream_attempts` | Payload transport calls during this invocation |
| `upstream_payload_bytes` | Payload bytes written from upstream, including partial progress written during the invocation |
| `local_payload_bytes` | Payload bytes copied from validated previous/earlier selected files |
| `files`, `recorded_bytes` | Completed recorded file count/bytes, excluding generated inventory itself |
| `resumed` | Whether explicit resume was requested |

These are not total wire traffic, disk usage or a freshness certificate. HTTP/TLS
headers, metadata traffic, assembly copies and already-verified workspace cache
reuse are not all counted as payload bytes. A failed invocation has no completed
report; an older completed report is not evidence a new attempt succeeded.

Use `publish --preflight` for the no-write route. It returns `schema`, `files`,
`local_bytes`, `inventory_sha256` and `absent_paths`; actual publish always repeats
checks. Upload/GET progress and typed results are available in events mode.

Publication report fields are `schema` (source inventory schema), `files` (includes
its uploaded inventory marker), `remote_bytes_verified` (GET-verified file bytes,
not TLS traffic) and `inventory_sha256` (hash of the source inventory).

## Storage, budgets and integrity limits

| Bound | Current contract |
| --- | --- |
| Config/plan metadata | Up to 1 MiB |
| Upstream Rust manifest | Up to 4 MiB |
| Signature metadata | Up to 64 KiB |
| One archive/download | Up to 512 MiB |
| Selected plan | Up to 1,024 payload files |
| Combined generation | Up to 4,096 files, 256 retained toolchain records and 1 MiB generated inventory |
| Rustup finishing | At most 64 distinct recorded hosts and one promotion each for stable/beta/nightly |
| Publisher CA bundle | Up to 4 MiB and 256 certificates |
| Byte budgets | Default 10 GiB; at most 100 GiB; exact byte integer, not strings such as `10G` |

A byte budget is not an OS disk quota. Sync limits combined selected working copies
and final generation separately. Temporary staging can approach twice its budget,
plus metadata/inventories and the separate previous input. Finishing copies its
retained input and adds installer/current-path copies; allow corresponding staging
and input disk space. All advertised formats are fetched for selected components.
Unknown sizes can cause a later refusal rather than exposing a completed tree.

Inventory schemas 1/2/3 are generated file/data descriptions, not cryptographic
signatures. Hashes detect mismatched bytes, not an administrator replacing a whole
inventory/plan. Retained upstream OpenPGP signatures are not independently checked.
Use trusted private local parents/transfers; hostile concurrent changes, NFS/shared
multi-machine caches and power-loss durability are not certified.

## Using the Rust library

The CLI is a thin layer over the same reusable core. It does not require a GUI.
Before publication, a consumer can use a local source dependency; after publication
use the crate version. These are dependency examples for a separate application:

```toml
[dependencies]
rust-mirror = { path = "../rust-mirror" }
```

After publication replace that declaration with `rust-mirror = "0.1.0"`.
Generate full type/signature documentation from the source with
`cargo doc --locked --no-deps`, then open `target/doc/rust_mirror/index.html`.

| Public API | Responsibility |
| --- | --- |
| `Result`, `Source`, `HttpsSource`, `Transfer` | Shared errors, injected source transport, official HTTPS implementation, range completion/restart result |
| Root `Options`, `prepare`, `Snapshot`, `SnapshotFile` | Legacy minimal GNU Linux preparation and file metadata |
| `selection::Config::{parse, read, validate, get}`, `Selection::validate` | Parse/validate saved configuration and look up named selections |
| `selection::catalog`, `Catalog`, `Availability`, `ComponentAvailability` | Release/host/component discovery data |
| `selection::import_toolchain`, `write_config` | Project declaration import and new-config writing |
| `selection::resolve`, `ResolvedPlan::save`, `load_plan`, `Plan`, `Artifact` | Resolve/freeze/persist/revalidate a selection plan |
| `selection::fetch`, `fetch_resuming`, `SelectionSnapshot` | New/resumed validated selected downloads |
| `selection::collection::assemble`, `ToolchainRecord`, `CollectionSnapshot` | Local retained-generation assembly and records |
| `selection::sync::run`, `run_forced`, `SyncResult`, `Report`, `SelectionStatus` | One-shot workflow, explicit destructive reset, results/counters |
| `selection::rustup::prepare` | Complete a new generation with pinned manager endpoints and promotion |
| `selection::retention::{discover, preview_directory}`, `Catalog`, `GenerationInfo`, `RunInfo`, `RunState`, `CatalogIssue` | Metadata-only local listing, automatic next-previous/paused dependency protection and directory-bound dry runs |
| `selection::retention::{preview, apply}`, `Policy`, `PrunePlan`, `Deletion`, `FolderDeletion`, `KeptGeneration`, `ApplyReport`, `ApplyFailure` | Local generation policy, structured exact preview, explicit validated deletion and partial-failure results |
| `publish::publish`, `publish::Options`, `publish::Report` | Validate/upload/verify a fresh remote prefix |

For example, prepare the second walkthrough configuration through the library:

```rust
use std::path::Path;
use rust_mirror::{selection, HttpsSource, Result};

fn main() -> Result<()> {
    let config = selection::Config::read(Path::new("mirror-002.toml"))?;
    let source = HttpsSource::default();
    let raw = selection::sync::run(
        &config, None, Path::new("library-run"), false, &source,
    )?;
    let promotions = vec!["stable-2025-09-18".to_owned()];
    let complete = selection::rustup::prepare(
        &raw.mirror, "1.29.3", &promotions, Path::new("library-complete"),
        config.max_download_bytes, &source,
    )?;
    println!("{}", complete.display());
    Ok(())
}
```

That example performs network/local writes when run and requires new output paths.
It does not publish. `sync::run`'s boolean is resume; use the separately named
`run_forced` only when destructive reset is intended. Some core functions emit
progress on stderr; library use is not a guarantee of silent/no-side-effect work.

For publication, supply every `publish::Options` field explicitly: `base_url`,
`ca_file`, `credentials: Option<(String, String)>`, and `allow_loopback_http` (normally
false). Read secrets through your application's own mechanism, not by expecting
implicit library environment behavior. `Source::fetch` is required for a custom
transport; `fetch_from` adds optional range handling using `Transfer`. Callers own
custom transport authentication/trust; `Source` must honor the supplied URL and
byte limit, and `Transfer::Restart` must write no bytes. Integrity checks do not
create an independent trust root. Keep the same path/byte/immutability boundaries as the CLI.

## Controlled library operations

`events::{Observer, CancelToken, Control, JsonLines, Event, EventKind,
PROTOCOL_VERSION}` share the CLI progress model. Observers are thread-safe, quick
callbacks; do not reenter the same control. `_with_control` variants of sync run/
forced run, fetch/resuming fetch, collection assemble, rustup prepare, publish/
preflight and retention previews/apply take `&Control`. The calling-thread context
propagates through private helpers; heartbeat emission runs independently. Existing
convenience signatures remain usable. The library installs no process signal handler.
Root `Error`, `ErrorKind` and `error_kind(&*error)` expose categories without parsing
messages. Controlled errors normalize to `Error`; custom legacy errors without a
known category become Invalid. `Error::report` can carry partial prune evidence.

`cli::run_with_events(args, writer, cancel)` is a shared adapter with hello/result
framing. Its writer is owned, Send and static; it returns 0/1/130 without installing
signal handlers. Standalone execution adds its binary-only signal bridge. Detailed
contracts and names are in [EVENTS.md](EVENTS.md).

## QA tools, compatibility and release status

The Python publisher is a reference/QA interface; it is not required by installed
`rust-mirror publish`. Proof scripts intentionally use private homes, temporary CA
material and disposable containers/hosted VMs. They do not configure a production
Nexus instance, accept EULAs or install test trust on ordinary workstations.

[Current results](evals/2026-11-11-rustup-lifecycle.md) cover four native hosts and
isolated legacy Nexus OSS 3.76.1-01, with setup failures and cleanup bounds retained.
A source inspection or passed Markdown check is not new runtime evidence. Native
SDKs, old/unlisted OS versions, authenticated client gateways and production
policies retain the [user-guide limits](USER-GUIDE.md#limits-demos-and-further-reading).

Scheduling, Nexus pruning, in-generation compaction, Cargo registry/index mirroring and rust-mirror-gui are future
work. See the [release notes](releases/v0.1.0.md) for actual publication state and
[release checklist](RELEASING.md) for the remaining authorized-release process.

## Corporate network options (0.3.0)

See the [network guide](NETWORK.md) for routing, precedence, credentials, TLS trust,
cache format/limits and library API details. No settings are discovered implicitly.
All upstream commands (`list`, `plan`, `fetch`, `sync`, `rustup`, `fetch-toolchain`)
accept the global options below; they may appear before or after the command.
`check-network` and `releases` use them too. Publish networking requires its own
explicit flags or `--network-config`; upstream settings are never inherited.

| Option | Meaning and default |
| --- | --- |
| `--proxy URL` | Explicit credential-free `http://host:port` CONNECT proxy; default direct |
| `--proxy-from-env` | Opt in to HTTPS_PROXY/https_proxy/ALL_PROXY/all_proxy precedence and NO_PROXY/no_proxy |
| `--no-proxy LIST` | Comma-separated exact hosts/IPs, domain suffixes, optional ports or `*`; default none |
| `--proxy-username-env NAME` | Explicit username variable name; pair with a password source |
| `--proxy-password-env NAME` | Explicit password variable name; never pass the value in argv |
| `--proxy-password-stdin` | Read one bounded UTF-8 password line before optional cancellation input; conflicts with a password variable |
| `--extra-ca-file PEM` | Add PEM certificates to Mozilla roots; default no extra CA, never disable TLS |
| `--connect-timeout SECONDS` | Positive socket-connect seconds, default 180, maximum 86400; OS DNS is separate |
| `--read-timeout SECONDS` | Positive per-read/write seconds, default 180, maximum 86400; not an overall deadline |
| `--network-config FILE` | Load `[network]` from a validated mirror TOML; CLI settings override it |
| `--estimate-sizes` | Plan/sync only; opt-in HEAD estimates, default off; no payload download for measurement |

Plan with estimates returns `plan` plus `sizes`; without it the existing Plan output
is unchanged. Estimates are not saved into frozen plans or used as hash evidence.
Sync estimates emit protocol-1 `transfer_summary`, with nullable totals/rate/ETA;
resume still requires the same frozen payload recipe and previous generation.
`Config.network` is not part of that recipe, allowing explicit route/trust changes.

### check-network

`rust-mirror [--events jsonl] check-network [NETWORK_OPTIONS]` reads connection
settings and optional CA/secret inputs, probes DNS/connect/proxy auth/TLS and fetches
a small official checksum file. No local mirror or remote publication is written.
Returns ordered passed/failed/skipped steps, typed failure kinds and actually
presented certificate metadata when available. No CA is automatically trusted.
On failure exit 1 with the diagnostic report in `error.report`; cancellation exits
130. Correct settings and repeat explicitly; no automatic retries occur.

### releases

`rust-mirror [--events jsonl] releases --cache DIRECTORY [--refresh | --offline]`
loads the bounded official root manifest index and lists stable versions/dates and
beta/nightly selectors. `--refresh` requires network success; `--offline` reads an
existing valid cache with zero source requests. Default attempts refresh and may
fall back to valid cached bytes with a typed warning. Reports include cache age,
source and original-index SHA-256. Index entries do not prove component availability.
Cache directories must be new or contain only managed regular cache/lock files;
corrupt, linked, unrelated or competing-writer state is refused. Cache writes use a
locked atomic replacement; inspect/repair invalid state explicitly before retrying.
No mirror, payload archive or remote server is written. Cancellation preserves any
previous complete cache. See the [cache contract](NETWORK.md#release-discovery-and-cache).

## Crate mirroring MVP (0.4.0)

These experimental commands are introduced in 0.4.0; see its [release notes](releases/v0.4.0.md).
Use a dedicated [crate catalog](CRATES.md); native planning never invokes Cargo.
All accept protocol-1 `--events jsonl`; network flags apply to plan/fetch/sync.

| Command | Inputs, defaults and effects |
| --- | --- |
| `crates catalog --config FILE` | Locally parse/normalize a catalog; human output is canonical TOML, event result is typed fields. Does not write it. |
| `crates paste --input FILE [--output NEW_FILE]` | Read a standalone dependencies block, report all accepted/rejected entries. Write only a new catalog and only with no refusals; never approve downloads. |
| `crates plan --config FILE --output NEW_PLAN.json [--previous MIRROR] [--estimate-sizes]` | Fetch bounded official sparse metadata; optional archive HEAD estimates. Freeze original records, closure, base URL, budgets and predecessor identity. No archive GET, output overwrite or implicit approval. |
| `crates show --plan FILE` | Bounded local reload/validation of the exact preview, with no network/environment access. |
| `crates explain --plan FILE --crate NAME [--version VERSION] [--limit N]` | Local structured inclusion chains; default 64, maximum 256 returned chains. Reports truncation and nullable total when traversal is capped. |
| `crates fetch --plan FILE --output NEW_RUN --approve-plan ID [--previous MIRROR] [--resume]` | Require whole-preview identity; stream into locked private state, verify upstream checksums, then expose immutable `mirror/`. Previous is mandatory exactly when frozen. Resume requires matching initialized state; a completed tree is revalidated, never overwritten. |
| `crates sync --config FILE --output RUN [--previous MIRROR] [--approve-plan ID] [--resume] [--estimate-sizes]` | Without consent, save/return `needs_approval` plus the whole preview. With consent, reuse that exact saved plan; no index refresh. Payload work requires explicit resume after initialization. Changed config/previous refuses. |
| `crates generations --directory DIRECTORY` | Metadata-only discovery with crate/version counts, run state and predecessor identities. Does not establish archive integrity. |
| `crates prune --config FILE --directory DIRECTORY --next-previous MIRROR --as-of YYYY-MM-DD [--dry-run \| --apply]` | Read/hash bounded candidates and show exact whole-generation deletion paths/bytes. Dry-run is default; apply explicitly authorizes revalidated locked deletion. Crate policy and pins, current, next-previous and paused-run dependencies protect closures. No remote pruning. |

Existing `publish --source MIRROR --base-url URL [--preflight]` accepts schema 4
with the same independent publisher network/auth settings. It refuses a different
generation base URL before remote requests, checks every absence before writes,
verifies GET hashes and sends the marker last. Partial uploads require investigation
and a fresh prefix; no overwrite, activation or rollback is implied.

## Lock source commands (development source for 0.5.0)

| Command | Effects and refusal behavior |
| --- | --- |
| `crates import-lock --config FILE --lock NAME=PATH [--lock NAME=PATH ...] [--write]` | Local UTF-8 regular-file analysis only (16 MiB/file, 64 MiB/batch, 128 names); default preview returns proposed catalog, typed diagnostics and source/pin diffs. Explicit write atomically replaces the valid catalog after successful whole-transaction analysis; no partial source write. Copies provenance and all exact crates.io pins; manual rules remain. No network or archive requests. |
| `crates remove-lock --config FILE --source NAME [--source NAME ...] [--write]` | Preview existing-source removals; explicit write saves only those changes. Unknown/duplicate names refuse. Does not prune stored archives/generations. |

`NAME=PATH` splits at the first equals sign; quoted Windows drive paths are valid.
Shared source-name and bounded-file-reader APIs are in `crates::lockfile`; callers
pass the existing `Control` to parsing/import/removal previews. Rejected CLI imports
emit typed final errors with the analysis in `error.report`; accepted writes set
`written: true`. See [the complete result/format contract](LOCK-IMPORT.md), including
legacy metadata checksums, skipped sources, checksum integrity and scoped yanks.

New writers emit frozen-plan schema 2 / crate-inventory schema 5; old schema 1/4
inputs remain readable. Old prune previews require regeneration. Keep protocol 1;
structured `keep_sources` explains lock/manual/pinned protection. A validated
completed run with no mirror is `pruned`, so a second prune does not mistake it for
paused work. Private workspaces remain intact. No remote pruning is added.
