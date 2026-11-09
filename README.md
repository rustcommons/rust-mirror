# rust-mirror

Rust toolchain and crate mirroring for offline development.

## Planned products

- **rust-mirror**: the core mirroring tool and command-line interface, inspired by
  [Panamax](https://github.com/panamax-rs/panamax). Planned scope covers Rustup
  toolchain artifacts and Cargo crate mirroring.
- **rust-mirror-gui**: a future graphical interface for the same mirroring workflows.
  GUI implementation, framework, packaging, and release timing are not yet defined.

The names were selected on 2026-11-09. Neither crate has been published or reserved
on crates.io by this naming change.

## Current state

This repository currently contains documentation, a license, and ignore rules.
The planned products are not implemented here. No installation commands or runtime
compatibility guarantees are established.

An earlier, uncommitted Rustup mirror prototype was located in the owner's Windows
checkout on 2026-11-09. Its source, configuration, lockfile, and tests are preserved.
It has not been imported or qualified; assess it before deciding what to reuse.

The repository was renamed from `rustup-offline-mirror` to `rust-mirror`, preserving
its Git history. Existing `ROM-REQ` identifiers remain stable.

## Documentation

- [Recovered prototype assessment](docs/evals/2026-11-09-prototype-assessment.md).
- [Requirements and evidence index](docs/REQUIREMENTS.md).
- [Contribution and maintenance instructions](AGENTS.md).
- [Rust Commons documentation conventions](https://github.com/rustcommons/.github/blob/main/docs/DOCUMENTATION.md).
- [License](LICENSE).
