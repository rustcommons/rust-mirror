# rust-mirror

Prepare Rust toolchain snapshots for offline development and static hosting,
including Nexus Repository Raw hosted repositories.

## Current state

Experimental, unreleased CLI and reusable library. The first milestone prepares
a pinned minimal-profile x86_64 Linux toolchain and preserves upstream metadata.
A network-isolated proof bootstraps Rustup, installs Rust 1.90.0 and builds/runs a
dependency-free program from a local Nexus-style HTTP path. Actual Nexus/TLS/auth
integration has not been tested. See [hosting and limitations](docs/NEXUS-RAW.md)
and the [proof record](docs/evals/2026-11-09-offline-toolchain.md).

Rust/Cargo 1.90+ is required to build. This project is not yet published on crates.io.

```sh
cargo build --locked
cargo run --locked -- fetch-toolchain --release 1.90.0 --host x86_64-unknown-linux-gnu --rustup-version 1.28.2 --output ./snapshot-1.90.0
```

The new output directory's parent must exist. The completed static tree is under
`snapshot-1.90.0/mirror`. The [Nexus guide](docs/NEXUS-RAW.md) explains transfer,
verified HTTP PUT publication and client setup.

## Planned products

- **rust-mirror**: core library and CLI, inspired by [Panamax](https://github.com/panamax-rs/panamax).
  Broader Rustup workflows and Cargo crate/index mirroring remain planned.
- **rust-mirror-gui**: future interface over the same core. No GUI implementation
  or framework has been selected.

The repository was renamed from `rustup-offline-mirror` on 2026-11-09, preserving
Git history and stable `ROM-REQ` identifiers. The earlier Windows-local uncommitted
prototype remains untouched and separately preserved; its assessment is historical.

## Documentation

- [Nexus Raw hosting and offline installation](docs/NEXUS-RAW.md).
- [Offline proof evidence](docs/evals/2026-11-09-offline-toolchain.md).
- [Recovered prototype assessment](docs/evals/2026-11-09-prototype-assessment.md).
- [Requirements and evidence index](docs/REQUIREMENTS.md).
- [Changelog](CHANGELOG.md).
- [Contribution and validation instructions](AGENTS.md).
- [License](LICENSE).
