# Changelog

## [Unreleased]

## [0.4.0] - 2026-11-13

- Mirror organization-wide curated crate catalogs with Cargo-compatible registry
  dependency syntax, aliases, features, latest-N rules and exact pins. Resolve
  native all-platform normal/build closure without a runtime Cargo dependency.
- Preview counts, optional HEAD size estimates, budgets and inclusion chains in a
  frozen JSON plan. Require one explicit whole-preview approval before archive GETs.
- Fetch/sync with bounded concurrency, Retry-After/backoff, checksum verification,
  locks, cooperative cancellation and initialized-state resume. Publish immutable
  schema-4 static sparse generations to a fresh matching HTTPS/Nexus Raw prefix.
- Retain verified historical closures; add crate-aware whole-generation pruning
  with declared pins, exact previews, stale-state refusal and explicit apply.
  Refuse historical/current combinations requiring an unmirrored dependency edge.
- Add typed catalog JSON/TOML round-trips and paste diagnostics, metadata-only Cargo
  config snippets, crate discovery counts and additive protocol-1 completion,
  retry and file-counter events for library/GUI workers.
- Qualify the dated 100-root corpus and archive-byte overfetch. Real HTTPS Cargo
  client builds cover itoa on Linux/Windows/macOS; actual Nexus client evidence is
  Linux-only. These bounds do not imply arbitrary crates/targets/features work.
- Include the previously approved native Phase 0 research API/conformance tooling
  and direct `semver`/`serde_json` dependencies. No runtime Cargo, third-party
  solver, tar or flate2 dependency is introduced. Rust/Cargo 1.90+ remains required.

## [0.3.0] - 2026-11-12

- Add explicit upstream HTTP CONNECT proxy/auth, NO_PROXY exclusions, environment
  opt-in, additive corporate CA bundles and connect/read timeouts. Keep official
  origins, no redirects and the independent direct publisher default.
- Add staged `check-network` diagnostics and public certificate/CA metadata APIs;
  display the presented issuer/chain on TLS refusal without bypassing verification.
- Add opt-in plan HEAD estimates and sync aggregate transfer/rate/ETA events,
  retaining actual byte budgets, frozen payload identity and hash verification.
- Add official stable/beta/nightly release discovery with explicit refresh,
  zero-network offline cache reads, cache age and typed fallback warnings.
- Add public `NetworkOptions` and separately opted-in publisher networking. Existing
  library entry points remain; `Config` literals need optional `network`.
- Keep proxy credentials in memory or explicitly selected secret inputs, never
  saved values in config/argv/events. Protocol 1 gains additive progress events.

- Record owner approval (2026-11-12) for direct `webpki-roots` and `x509-parser`
  0.18 with default features disabled for corporate-network support. Certificate
  metadata is display-only; rustls retains TLS trust decisions.

## [0.2.0] - 2026-11-11

- Add protocol-1 JSON lines for CLI/GUI operation, independent heartbeat/watchdog
  context, typed outcomes and opt-in stdin/EOF cancellation. Existing human mode remains.
- Add library observers/cancellation controls and safe checkpoints during transfers,
  copies, hashing and pruning. Preserve initialized fetch/sync state for explicit resume.
- Add no-write publish preflight and upload/GET verification progress. Publication
  still refuses existing paths and may leave partial remote data after interruption.
- Shorten README/docs.rs entry points and link maintained detailed guides.
- Add explicit local generation retention for the planned 0.2.0 feature: declared
  release pins, latest stable counts, nightly calendar windows and rollback roots.
- Add read-only `prune` previews with exact file paths, hashes and sizes; deletion
  requires `--apply`, locks and revalidates the plan, and reports partial failures.
- Add metadata-only local generation/run discovery and directory-backed plans with
  keep-newest and generation pins, automatic next-previous/paused-run dependency
  protection, per-folder byte totals and managed-directory change refusal.
- Preserve incomplete/locked runs and private sync/fetch state. Whole-workspace
  cleanup and exact config-aware run inspection remains deferred.
- Expose structured library preview/apply results for GUI callers. Existing config
  TOML remains valid; Rust `Config` literals need the new optional `retention` field.
- Prune whole completed local generations only. Kept collections are not compacted;
  Nexus-side deletion remains deferred. No 0.2.0 publication is authorized here.

## [0.1.1] - 2026-11-11

Documentation-only patch. No command behavior, API signatures, dependency
requirements, MSRV or saved schemas change.

- Add a docs.rs usage walkthrough with configuration, preparation, Nexus publishing,
  client setup, recovery and compiled Rust examples.
- Describe all exported types, fields and operations; enable missing-documentation
  warnings so future public items must have descriptions.
- Organize the crates.io README around installation, first mirror, publishing,
  updates and reference links. Remove repository/prototype history.

See [versioned notes](https://github.com/rustcommons/rust-mirror/blob/main/docs/releases/v0.1.1.md).

## [0.1.0] - 2026-11-11

First release of selected modern Rustup mirroring and Nexus Raw publication.
See [versioned notes](https://github.com/rustcommons/rust-mirror/blob/main/docs/releases/v0.1.0.md) for availability and qualification.

- Add a complete operating guide and command/configuration reference, covering
  platform/version selection, private client examples, Nexus activation, recovery
  and current limitations.

- Add native verified HTTPS `publish` for Nexus Raw/static hosting, explicit private
  CA and scoped credential support, whole-prefix absence checks and remote hash
  verification. Add normal Rustup full/minor numeric lookup aliases and `--version`.

- Add `rustup` generation finishing: pinned self-update catalog, current/archive
  installers for selected hosts, original Unix installer and explicit dated channel
  promotion. Add native client lifecycle and expanded actual-Nexus proof drivers.

- Require an explicit choice for existing sync runs: `--resume` continues saved
  progress; `--force` discards the old managed run without backup and starts fresh.
  Reject combining them, active writers, unrelated outputs and previous-mirror overlap.

- Add one-shot `sync` across saved selections, frozen-cohort resume, verified local
  payload reuse, unchanged detection, stage summaries and structured run reports.
- Add a quick fixture demo and a recorded real two-version incremental/no-change demo.

- Add local `assemble` for immutable multi-version mirrors, with explicit previous
  generation retention and coherent moving-channel alias replacement.
- Validate combined inventories, artifact closure, conflicts and byte budgets before
  exposing schema 3 output; support verified publication and offline coexistence proofs.

- Add explicit `fetch --resume` for the same frozen plan/output, with verified-file
  reuse, byte-range continuation and safe whole-file restart when ranges are ignored.
- Preserve partial progress behind a single-writer output lock; verify final hashes
  before completion and recheck existing completed snapshots without overwriting them.

- Add named version/channel, host, profile, component and cross-target selections.
- Add release-specific discovery and explicit-host project toolchain-file import.
- Split metadata-only planning from verified frozen-plan downloads; mirror all
  advertised archive formats for selected components.
- Support schema 2 selected snapshots in the publisher and isolated Linux proof.

- Add an experimental CLI/library for pinned, minimal-profile Linux toolchain snapshots.
- Preserve upstream manifests and signature files; check downloaded SHA-256 values.
- Add static-host/Nexus Raw publication helper and isolated offline-install proof.
- Cargo crate mirroring and the GUI remain planned; scheduling is outside 0.1.0.

- Add publisher `--ca-file` for verified private-CA HTTPS connections.
- Add a disposable actual-Nexus OSS compatibility proof with scoped access, MIME-policy controls and isolated offline installation; keep the Community Edition EULA gate explicit.
