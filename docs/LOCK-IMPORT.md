# Cargo.lock import contract (planned 0.5.0)

Owner-approved implementation scope, 2026-11-13. This is the contract for review
before implementation; 0.4.0 does not provide these APIs/commands. Publication
requires separate owner authorization. Existing [crate behavior](CRATES.md),
[protocol 1](EVENTS.md) and qualification gates continue to apply. No new dependency
or runtime Cargo invocation is approved.

## Self-contained catalog sources

Add `Catalog.crate_locks`, JSON/TOML key `crate-locks`: a source-name-keyed map.
Names are 1–128 ASCII letters/digits/`_`/`-`/`.` (excluding `.` and `..`); names
are case-sensitive and independent of a machine's paths. Example fragment:

```toml
[crate-locks.service-a]
file-name = "Cargo.lock"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
imported-at = 1794528000
lock-version = 4

[[crate-locks.service-a.pins]]
name = "itoa"
version = "1.0.15"
checksum = "4a5f13b858c8d314ee3e8f639011f7ccefe71f97f96e50151fb991f267928e2c"
dependencies = []
```

`sha256` hashes original input bytes, not reserialized TOML; the example digest/time
are illustrative. `file-name` is a bounded basename, never an absolute path.
`imported-at` is UTC Unix seconds, supplied explicitly to the pure library operation.
Each pin has exact registry name/version/checksum and copied resolved dependency
identities. Normalize the two accepted upstream identities to `crates-io` internally.
A dependency identity contains `name`, `version`, `source_kind` (`crates_io`, `git`,
`local`); git/local placeholders carry no URL or filesystem path. Persist those
placeholders and their referenced registry edges as `external_packages`, so the
resolved graph remains checkable without the original file. They are not archives.

The catalog contains the complete exact pin set, resolved edges and provenance;
never retain a lockfile path to reread during planning. Lock-only catalogs are valid
with an empty `crates` table; existing explicit coverage/output/budget policy remains
required before planning. Canonical TOML/serde round-trip includes all source data;
rewriting still loses comments/formatting as documented for catalog editing.

Import unions with manual declarations/rules and other named sources. Reimport
replaces only the named source, returning added/removed/unchanged pin identities
and provenance changes; omitted source names remain untouched. Removing a source
removes only its protection/contribution. Neither operation deletes mirrored bytes,
changes a manual rule, downloads an archive or approves a mirror plan. Reimporting
identical bytes can update import time; any changed catalog identity invalidates
an old approval/resume identity. Same name/version with different checksums anywhere
in the catalog is an integrity refusal, never last-writer-wins.

## Parser, diagnostics and exact-lock validation

- Read standard Cargo.lock formats 1–4, including legacy `[root]` and abbreviated
  dependency references. Without a version header, infer legacy v1/v2 from Cargo's
  checksum/edge encoding; standard v1/v2 omit the header, v3/v4 include it.
  Nonstandard explicit headers 1/2, unsupported versions and contradictory
  encodings refuse with a typed reason.
- Accept only `registry+https://github.com/rust-lang/crates.io-index` and
  `sparse+https://index.crates.io/`. Do not follow URLs, normalize a different host,
  accept embedded credentials or substitute an alternate registry. Report every
  alternate registry with a typed, sanitized reason; the import transaction refuses.
- Copy every resolved crates.io package, including normal/build/dev and inactive
  target packages. Source-less entries are `local` (lockfiles cannot distinguish
  path dependencies from workspace members); git entries are `git`. Report each
  skip. Unused patch entries are reported as `unused_patch`, not resolved pins.
- Use inline checksums for v2–v4 and v1 `[metadata]` keys of the form
  `checksum NAME VERSION (SOURCE)`. Require a 64-hex SHA-256 for every registry pin.
  For registry pins, missing values, legacy `<none>` or conflicting inline/metadata
  values refuse. Ambiguous duplicate packages or unresolved dependency identities
  also refuse; never repair from
  the network or silently omit a registry entry. Unrelated metadata is not copied.
- Parsing/import is local only. Planning fetches bounded official index metadata and
  requires every pin's index version and exact `cksum`; mismatch is `integrity`.
  Missing/unsupported index records refuse per crate. Preserve original line bytes. Plan
  errors include `error.report { kind: lock_checksum_mismatch|lock_missing_version|
  lock_inconsistent|lock_unsupported_index, source_name, name, version,
  expected_checksum, actual_checksum, parent, requirement }` with nullable context;
  `lock_checksum_mismatch` uses top-level `integrity`, others `invalid`.
- Check each source's exact resolved registry edges against the proposed served
  index, including compatible requirements, renamed packages and required normal/
  build edges. Do not activate synthetic default features for every locked package
  or infer dev/feature/target profiles that the lock does not record. Optional edges
  absent from the lock do not expand it. Missing/inconsistent edges refuse with
  parent/required identity; no lock-source closure repair or archive expansion.
  Independent lock sources need not unify or share one `links` namespace.
- New yanked versions are permitted only as checksum-verified lock-imported exact
  pins. Mark them `yanked` and `locked_yanked` in preview. This is source-scoped
  permission: manual floating selections/manual new pins keep their existing yank
  refusal even if the same bytes are required by a lock source. Retained historical
  records keep their existing rule. Combined-index replay for manual/historical
  cohorts remains mandatory; an unmirrored edge still refuses.

A lock does not prove build success or capture project manifests/SDKs/git sources.
The guarantee is availability of every imported registry archive with matching bytes,
subject to the explicit consistency gate. `external_sources_required` is true when
skips exist; a fresh Cargo home cannot fetch skipped git dependencies from this
registry. No claim that the registry alone supplies the whole mixed-source project.

Bounds: 16 MiB per input, 64 MiB per import batch, 128 sources, 10,000 packages per
input, 40,000 resolved edges per input, 10,000 unique imported registry pins in a
catalog, 4,096-byte diagnostic messages and 1,024 diagnostics per report. Reports
carry `total_diagnostics`/`truncated`; overflow refuses, never silently drops pins.
Raise catalog text capacity from 4 MiB to 64 MiB for copied sources. Existing plan,
metadata, version/file/byte, cohort and native-search budgets still apply; exceeding
any refuses. Check cancellation while reading/parsing entries and between index
requests; independent heartbeats remain active during blocking work.

## Library, CLI and result shapes

Public planned module `crates::lockfile` uses existing `Control`, `ErrorKind` and serde:

| API / type | Contract |
| --- | --- |
| `LockInput { source_name, file_name, text }` | UTF-8 input already bounded; CLI reads only explicit regular files, refusing symlinks. No directory discovery. |
| `parse_with_control(input, control) -> Result<ParseReport>` | Pure analysis with inferred version, exact pins/edges, skipped entries and diagnostics; no mutation/network. |
| `preview_import(catalog, inputs, imported_at, control) -> Result<ImportReport>` | All inputs analyzed as one transaction; proposed cloned catalog only when accepted. Duplicate source names in a batch refuse. |
| `preview_remove(catalog, source_names, control) -> Result<ImportReport>` | Explicit existing names only; proposed catalog and source/pin diff; no file writes. |
| `Catalog::write(path, replace)` | Existing explicit canonical atomic write; GUI does not write TOML itself. |

`ImportReport { schema: 1, accepted, catalog: Catalog|null, sources, diagnostics,
total_diagnostics, truncated, external_sources_required, written }` is serializable.
`SourceImportReport { source_name, action: added|replaced|removed|unchanged,
file_name, sha256, imported_at, lock_version, added, removed, unchanged, skipped,
provenance_changed }`; pin lists carry name/version/checksum, not prose.
`SkippedEntry { name, version, kind: git|local|unused_patch, line, column }`.
Diagnostics carry source name, nullable package/field, one-based line/column when
available, nullable credential-free `registry_origin`, typed `kind` and sanitized
message. Each refused registry entry is listed; never echo its raw URL. Kinds: `syntax`, `unsupported_version`,
`invalid_package`, `invalid_checksum`, `missing_checksum`, `conflicting_checksum`,
`duplicate_package`, `alternate_registry`, `ambiguous_dependency`, `missing_dependency`,
`unknown_field`, `budget`. Never echo credential-bearing source URLs.

Analysis can return `accepted: false` without an opaque Rust error; no proposed
catalog is available then. I/O/cancellation errors use existing public error kinds.
CLI maps rejected analysis to final `invalid` (or `integrity` for conflicting
checksums), with the full typed report in `error.report`; successful preview returns
that same report in `result.report`. No partial accepted-source write is allowed.

```sh
rust-mirror crates import-lock --config crates.toml --lock service-a=project-a/Cargo.lock --lock service-b=project-b/Cargo.lock
rust-mirror crates import-lock --config crates.toml --lock service-a=project-a/Cargo.lock --write
rust-mirror crates remove-lock --config crates.toml --source service-a --write
```

Repeated `--lock NAME=PATH` splits only at the first `=` (Windows drive paths work).
Pure previews set `written: false`; successful CLI writes set it true.
Default is read-only preview/diff; `--write` explicitly commits the accepted catalog,
using the existing atomic canonical writer and its symlink/path refusals. Remove defaults to
preview too. No external lock path is persisted; the source digest is provenance,
not an independently verifiable signature of an unavailable original file.

## Frozen preview, events and retention

Version rows add `sources: [{ kind: manual|lock, name, digest: string|null }]` and
`locked_yanked: bool`; keep `root_aliases` for existing manual consumers. Manual
transitive reasons identify their originating aliases; lock reasons identify source
name/digest and exact pin. Explain returns these same objects with existing bounded
chains/counts/truncation. Shared versions list all source reasons deterministically;
provenance-only changes are reported separately from archive additions/removals.

New writers use FrozenPlan schema 2 and crate inventory schema 5, carrying copied
lock-source proofs; 0.5 readers continue verifying old schema-1 plans/schema-4
inventories. 0.4 readers refuse new schemas. Protocol remains 1 with additive fields.
Freeze source digests/pins/graph, index observations and consistency results into
plan identity; saved preview/replay/resume never reopens the source lockfile.

Current catalog lock pins protect the newest fully verified generation covering
that source's entire exact pin/checksum set and locked edge proof; if none exists,
pruning refuses with source context. Shared/manual protection is additive. Removal
or replacement does not delete historical generations or retained archives; existing
current/next-previous/paused-run locks, explicit preview/apply and stale-state checks
remain. Retained lock cohorts keep independent exact proofs, not invented features.

## Qualification before implementation acceptance

Pin real-project commits, lockfile SHA-256, license/source links and tool versions:
several projects including git/path entries, a real locked yanked crate, old v1/v2
and current v4; add v3, malformed/checksum/alternate-source/abbreviation fixtures.
Use real Cargo `fetch --locked` then `build --locked --offline` against the served
HTTPS mirror in isolated Cargo homes on Linux AND Windows, preserving lock bytes.
For mixed-source projects, supply local paths and provision exact git checkouts
separately, with a missing-git control; never present that as mirrored git support.
Report platform/toolchain/SDK exclusions explicitly; invented toy locks alone do
not satisfy the real-lock gates. If Cargo needs a lock migration under `--locked`,
record the refusal and qualify a compatible client without silently rewriting it.

Prove dev/target packages copied, zero planning archives, checksum mismatch refusal,
yank-scoped permission, source replacement/removal/manual preservation, deterministic
diffs, bounds/line diagnostics, catalog round-trip, source deletion after import,
frozen replay/resume/cancellation, schema-4 compatibility and lock-pin retention.
Existing native/top-100, client, Nexus, resume, security and release gates remain.
No source implementation, version bump or publication is delivered by this contract.

## Primary references

- [Cargo 1.90 lock encoder/decoder](https://github.com/rust-lang/cargo/blob/840b83a10fb0e039a83f4d70ad032892c287570a/src/cargo/core/resolver/encode.rs): legacy root/metadata checksums, abbreviated identities and absent feature/dependency-kind context.
- [Cargo resolver](https://doc.rust-lang.org/cargo/reference/resolver.html#yanked-versions): existing locked yanked versions versus new resolution.
- [Cargo source replacement](https://doc.rust-lang.org/cargo/reference/source-replacement.html): byte-equivalent registry subsets and independent git sources.
- [Cargo command locking/offline semantics](https://doc.rust-lang.org/cargo/commands/cargo.html#manifest-options): existing lock preservation and offline behavior.
