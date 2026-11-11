# rust-mirror

Prepare Rust toolchain snapshots for offline development and static hosting,
including Nexus Repository Raw hosted repositories.

## Current state

Experimental, unreleased CLI and reusable library. Choose named toolchain
versions/channels, host platforms, profiles, components and compilation targets;
preview a metadata-only plan, then download that frozen selection for static hosting.
Interrupted selected downloads can [resume verified progress](docs/RESUMING.md).
Combine completed selections into an [immutable multi-version mirror](docs/MULTI-VERSION.md).
Publish a verified generation with the native `publish` command; Python is needed
only for the optional reference helper and qualification scripts.
Rustup remains the installer. See the [selection guide](docs/SELECTIONS.md).
Use [one-shot sync and runnable demos](docs/SYNC.md) to run the workflow with one command.
[Complete Rustup endpoints and channel promotion](docs/RUSTUP.md) are being qualified
for the [0.1.0 release gates](docs/RELEASE-0.1.0-PLAN.md).

Rust/Cargo 1.90+ is required to build. This project is not yet published on crates.io.

```sh
cargo build --locked
# Use target/debug/rust-mirror (Windows: target/debug/rust-mirror.exe).
rust-mirror init --config mirror.toml
rust-mirror list --toolchain 1.90.0 --host x86_64-unknown-linux-gnu
# Edit mirror.toml before planning; linux-dev is one of the example selections.
rust-mirror plan --config mirror.toml --selection linux-dev --output ./linux-plan
rust-mirror fetch --plan ./linux-plan --output ./linux-snapshot
```

Output parents must exist; existing outputs require an explicit same-plan `--resume`. The completed static tree
is `linux-snapshot/mirror`. [Import a project's rust-toolchain.toml](docs/SELECTIONS.md#import-a-projects-toolchain-file)
to avoid manually duplicating its components and targets.

The [selection proof](docs/evals/2026-11-09-toolchain-selection.md) covers 22 real
metadata cases and a network-isolated Linux installation with Clippy, rustfmt,
Rust sources and a Wasm standard library. Other platform execution remains unqualified.
A separate [Nexus OSS compatibility test](docs/NEXUS-INTEGRATION.md) verified the
original minimal snapshot on a legacy server fixture with explicit Raw MIME policy.
Modern Community Edition testing is held for operator license onboarding; your
production setup remains unqualified. See [hosting and limitations](docs/NEXUS-RAW.md).

## Planned products

- **rust-mirror**: core library and CLI, inspired by [Panamax](https://github.com/panamax-rs/panamax).
  Broader Rustup workflows and Cargo crate/index mirroring remain planned.
- **rust-mirror-gui**: future interface over the same core. No GUI implementation
  or framework has been selected.

The repository was renamed from `rustup-offline-mirror` on 2026-11-09, preserving
Git history and stable `ROM-REQ` identifiers. The earlier Windows-local uncommitted
prototype remains untouched and separately preserved; its assessment is historical.

## Documentation

- [Start testing: one-shot sync and demos](docs/SYNC.md).
- [Multi-version generations and explicit channel refresh](docs/MULTI-VERSION.md).
- [Resumable downloads and recovery](docs/RESUMING.md).
- [Toolchain selections, discovery, import and frozen plans](docs/SELECTIONS.md).
- [Nexus Raw hosting and offline installation](docs/NEXUS-RAW.md).
- [Offline proof evidence](docs/evals/2026-11-09-offline-toolchain.md).
- [Recovered prototype assessment](docs/evals/2026-11-09-prototype-assessment.md).
- [Requirements and evidence index](docs/REQUIREMENTS.md).
- [Changelog](CHANGELOG.md).
- [Contribution and validation instructions](AGENTS.md).
- [License](LICENSE).
