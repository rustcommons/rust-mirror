# Changelog

## [Unreleased]

### Fixed

- Explicitly release all workspace and catalog locks on return or error: toolchain
  and crate fetch/sync, prune/generation ownership, catalog writes, release-cache
  writes and retention shared probes. A transient fork-inherited descriptor
  cannot cause a false busy refusal after completion. Investigated under [#85](https://github.com/rustcommons/rust-mirror/issues/85);
  the original intermittent failure has no diagnostic proving its cause.

### Repository

- Treat crate corpus inclusion reasons and other listed explanatory metadata as
  informational while preserving selection, checksum, served-index and Cargo
  checks, including `source_ordinal` because it controls served index-line order.
  Compare shared dependency membership per root; fast CI also qualifies
  every changed root with Cargo, keeping causal and over-selection guards intact.

- Freeze throwaway GnuPG test clocks so slow key generation cannot make the
  next simulated signing step run backwards in time.

- Sign release SHA256SUMS with the environment-protected Rust Commons OpenPGP
  key before publishing; add SHA256SUMS.asc after 0.6.1 with pinned-primary
  verification, isolated keyrings, safe retries and throwaway-key CI proofs.
  Refuse revoked signatures; accept expired signing keys only for signatures
  made before expiry, without rejecting good signatures because a sibling expired.
  Use preinstalled python3 without setup-python and runner rustup only after
  signing-secret cleanup; verify both signatures across subkey rotation. Earlier release layouts
  and their verification remain unchanged.

- Include results and scenario context in sync test assertion failures to help
  diagnose the intermittent incomplete-assembly resume failure tracked in
  [#85](https://github.com/rustcommons/rust-mirror/issues/85); behavior is unchanged.

- Track versioned top-crate, tricky and real-lock regression corpora with frozen
  replay, explicit upstream/regression classification, CI-only Cargo conformance,
  weekly reports and a corpus release gate; refreshes require reviewed PRs.
  Require exact before/after Cargo agreement for transitive upstream exemptions,
  fail unconfirmed roots as upstream-needs-review, reject per-root over-selection
  growth, require weekly qualification to cover all executed source/helpers,
  and restrict tracking-issue updates to labeled, marked bot-owned issues.

- Record the OpenPGP dependency/key approvals and preserved prototype; integration
  waits for an rPGP release that clears both dependency advisories without exceptions.
- Contain offline-proof uploads within the served directory and reject symlinked
  parents, with regression tests; require TLS 1.2 or newer in the test servers.
- Record the completed 0.6.1 publication and first real phase 2 pipeline run,
  including its initial CI gate refusal and public artifact verification.
- Use the unified `actions/attest` action for release provenance and SBOM
  attestations, preserving the existing signer identity and verification policy.
- Update the root and fuzz lockfiles to `url` 2.5.9 and companion
  `form_urlencoded` 1.2.3 for RUSTSEC-2026-0592, keeping cargo-deny enforced.
  Published 0.6.1 is not affected in practice: neither its source nor ureq 2.12.1
  calls the affected `Url::origin` or `url::quirks::origin` functions. This is
  dependency maintenance; no advisory or patch release is planned.

## [0.6.1] - 2026-11-22

### Fixed

- The default publisher now uses bounded connect and per-read/write idle timeouts,
  matching the existing explicit-network path. Progressing transfers no longer hit
  the old whole-request deadline after an upload is stored. A peer trickling bytes
  can keep a request alive: only each read or write is time-bounded, with no overall
  time cap or minimum transfer rate.
- Crate downloads checkpoint progress after 8 MiB or 2 seconds of progress and on
  completion, error or cancellation, instead of fsyncing a shared receipt for every
  chunk. Resume discards bytes beyond the saved durable offset, revalidates prefixes
  and checks the final checksum. Corrupt staged crates are re-downloaded after a
  durable offset reset, including receipts left by earlier attempts; staged
  completion clears the partial offset without resetting the transfer counter.
  Existing private receipts migrate on resume.
- Incremental crate plans tolerate upstream nonidentity metadata backfills such as
  `rust_version`, report computed typed diagnostics through
  `FrozenPlan::metadata_warnings()` and CLI preview/explain, and serve the new upstream line
  in the successor. Historical cohort proofs keep their original bytes; changes to
  name, version, checksum, dependencies, features, features2 or links still refuse.
  Yank-only observations retain their existing handling. Checksums are never merged.
  Existing public struct fields, saved-plan JSON and IDs stay unchanged for the
  same inputs; only CLI/event reports add presentation diagnostics.
- Toolchain sync `--resume` and `--force`, and selected fetch resume, accept the
  persistent empty prune lock left by a successful prune.

### Repository

- Adopt the Rust Commons standard release pipeline, including phase 2: one
  `Trusted publishing` workflow now gates on an annotated tag, green CI on `main`, a
  dated changelog entry and release notes; builds the crate and all five binaries with
  every existing qualification gate and signs that build's output in the same isolated
  reusable workflow before the owner's approval; publishes after approval; creates the
  GitHub Release from the committed notes as a draft, uploads all assets and then
  publishes it (a killed run resumes the draft; assets are never replaced); and verifies
  crates.io, every attestation, docs.rs, a clean install and the public Windows, GNU
  and musl downloads. Releases after 0.6.0 carry the `.crate`, five archives, one SBOM,
  one `SHA256SUMS` over all of them and one `rust-mirror-VERSION.intoto.jsonl`
  bundle. `release-binaries.yml` is retired; the late-approval wait is gone. See
  [the release flow](https://github.com/rustcommons/rust-mirror/blob/main/docs/RELEASING.md#standard-release-pipeline).
  No crate or behavior change.
- Least-privilege hardening of the release pipeline: nothing compiles in the publish
  job (`cargo package --no-verify`, a byte comparison with the attested crate, then
  `cargo publish --no-verify`), and the crates.io token is minted last and is visible
  only to the publish step; SBOM and crate tooling run in their own job, apart
  from the binary payload and SHA256SUMS; archive READMEs print the strict
  `gh attestation verify` command with signer and digests; fork pull requests skip
  the dry-run build; a release is marked latest only if it is the highest version.

## [0.6.0] - 2026-11-21

- Update locked dependencies: `sha2` 0.11, `toml` 1.2, `serde` 1.0.229,
  `tempfile` 3.27, `indexmap` 2.14, and a direct `webpki-roots` 1.0 requirement.
  The TLS stack stays at `rustls` 0.23.46, `rustls-webpki` 0.103.17 and
  `webpki-roots` 1.0.10, as in 0.5.4; `ureq` still pulls the `webpki-roots` 0.26
  compatibility shim, which re-exports the same 1.0 roots. No application source
  change and no new direct dependency.
- **Breaking (library):** `Error::report` is now `Option<toml::Value>` from `toml`
  1.x instead of 0.8, hence 0.6.0. Code that names that type through its own `toml`
  0.8 dependency must move to `toml` 1. The CLI and event protocol 1 are unchanged.
- First release whose GitHub assets include the signed provenance bundle
  `rust-mirror-VERSION.intoto.jsonl` for offline `gh attestation verify --bundle`.
  SHA256SUMS still covers the five archives and the SBOM.
  See [0.6.0 details](https://github.com/rustcommons/rust-mirror/blob/main/docs/releases/v0.6.0.md).

## [0.5.4] - 2026-11-16

- Deliver the first prebuilt downloads carrying the security-sensitive dependency
  fixes already published in the 0.5.3 crate. Version 0.5.3 is crate-only: its SBOM
  attestation failed before any binary upload. Use 0.5.4 downloads, or a patched
  0.5.3-or-later crate installed with --locked; older binaries remain unchanged.
- Fix release SBOM attestation input by adding a deterministic canonical UUID
  serialNumber. Keep five archives plus the Cargo CycloneDX SBOM and one six-entry
  SHA256SUMS, with reusable provenance and per-archive SBOM bindings.
- No application implementation or locked dependency identity changes from 0.5.3;
  only the reported package/tool version and user documentation change. Keep
  immutable tags, exact source/signer checks and historical release outcomes.
  See [0.5.4 details](https://github.com/rustcommons/rust-mirror/blob/main/docs/releases/v0.5.4.md).

## [0.5.3] - 2026-11-16

- Refresh locked rustls to 0.23.46, rustls-webpki to 0.103.17, webpki-roots to
  1.0.10 and rustix to 1.1.6 for eleven RustSec advisories. Required existing
  libc/linux-raw-sys entries also update; no new crate names or application logic.
  Upgrade <=0.5.2 locked installs/prebuilt downloads; see [security details](https://github.com/rustcommons/rust-mirror/blob/main/docs/releases/v0.5.3.md#security)
  for all GHSA/RustSec IDs, configuration exclusions and exposure bounds.
- Ship a source-bound Cargo CycloneDX SBOM alongside the five archives, with one
  SHA256SUMS covering all six payloads. Reusable build/sign isolation, provenance
  and archive/SBOM bindings replace the historical release signer for this profile.
  Scope /Brepro to Windows release builds alongside static CRT; retain actual
  two-clean-build measurements rather than assume universal reproducibility.
- Add weekly grouped dependency/action updates, pinned actions, RustSec/license/
  source gates, workflow security lint, Scorecard and owner-reviewed Trusted
  Publishing through trusted-publishing.yml. Keep the manual release path.
- Package/tool version is 0.5.3; protocol, schemas and mirroring APIs are unchanged.

## [0.5.2] - 2026-11-15

- Ship five prebuilt CLI downloads with one SHA256SUMS: Windows with static CRT,
  dynamic GNU/Linux targeting glibc 2.17 via pinned cargo-libc 0.7.0, static musl
  Linux, and both macOS architectures. All archives and the manifest receive GitHub
  provenance attestations; executables remain unsigned by Windows/Apple.
- Gate GNU downloads on launch and real HTTPS crate plan/sync in CentOS 7,
  AlmaLinux 8, openSUSE Leap 42.3, Ubuntu 16.04/18.04/20.04 and Debian 8/9 container
  userlands. Old kernels are untested; glibc below 2.17 should use static musl.
- Put binary downloads first in install guidance, with checksum/provenance checks,
  runtime requirements and unsigned-download prompts. Preserve v0.5.1's four
  archives and original manifest unchanged.
- No mirroring runtime logic, dependencies, schemas or protocol changes from
  0.5.1; the reported tool/package version changes to 0.5.2.

## [0.5.1] - 2026-11-15

- Allow usable crate previews when an upstream index contains historical,
  unselected versions that differ only in build metadata. Report typed collision
  warnings instead of rejecting the entire index, including the old ndk-sys
  records reached through windowing dependencies.
- Refuse selected, lock-pinned or retained normalized versions with conflicting
  checksums, naming the crate, original versions and checksums. Keep full identities
  and original index lines; never merge conflicting records or bypass yank rules.
- Carry bounded warning observations in frozen previews and retained proofs while
  preserving identities of older plans whose new warning fields are empty.

## [0.5.0] - 2026-11-13

- Import Cargo.lock formats 1–4 as copied named pin/graph sources with digest/time
  provenance and typed replacement/removal diffs. Keep manual catalog rules;
  checksum-check every exact registry pin, including dev/target packages and scoped
  locked yanks, without runtime Cargo or lock-source closure expansion.
- Add public bounded file/name helpers, source-aware preview/explain and structured
  lock/manual/pinned retention reasons; write frozen schema 2 / inventory schema 5
  while retaining old schema 1/4 readers.
- Fix repeated crate pruning: completed receipts with an entirely absent mirror
  are recognized as pruned rather than mistaken for paused runs needing deleted
  predecessors. Private plans/receipts remain intact; partial/active runs stay protected.

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
