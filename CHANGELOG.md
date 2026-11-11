# Changelog

## [Unreleased]

No additional changes queued.

## [0.1.0] - 2026-11-11

First release of selected modern Rustup mirroring and Nexus Raw publication.
See [versioned notes](docs/releases/v0.1.0.md) for availability and qualification.

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
