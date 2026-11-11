# rust-mirror

Prepare Rust toolchain snapshots for offline development and static hosting,
including Nexus Repository Raw hosted repositories.

## Current state

CLI and reusable library for selected modern Rustup distributions. Choose named toolchain
versions/channels, host platforms, profiles, components and compilation targets;
preview a metadata-only plan, then download that frozen selection for static hosting.
Interrupted selected downloads can [resume verified progress](https://github.com/rustcommons/rust-mirror/blob/main/docs/RESUMING.md).
Combine completed selections into an [immutable multi-version mirror](https://github.com/rustcommons/rust-mirror/blob/main/docs/MULTI-VERSION.md).
Publish a verified generation with the native `publish` command; Python is needed
only for the optional reference helper and qualification scripts.
Rustup remains the installer. See the [selection guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/SELECTIONS.md).
Use [one-shot sync and runnable demos](https://github.com/rustcommons/rust-mirror/blob/main/docs/SYNC.md) to run the workflow with one command.
[Complete Rustup endpoints and channel promotion](https://github.com/rustcommons/rust-mirror/blob/main/docs/RUSTUP.md) cover bootstrap,
self-update and explicit client-generation changes. The [release record](https://github.com/rustcommons/rust-mirror/blob/main/docs/RELEASE-0.1.0-PLAN.md)
states exact qualification and remaining limits.

Rust/Cargo 1.90+ is required to build. Install a published version with
`cargo install rust-mirror --version 0.1.0 --locked`, or build the source below.
Check the [release notes](https://github.com/rustcommons/rust-mirror/blob/main/docs/releases/v0.1.0.md) for availability and exact evidence.

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
is `linux-snapshot/mirror`. [Import a project's rust-toolchain.toml](https://github.com/rustcommons/rust-mirror/blob/main/docs/SELECTIONS.md#import-a-projects-toolchain-file)
to avoid manually duplicating its components and targets.

The [Rustup lifecycle guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/RUSTUP.md) explains fresh clients, existing-client
updates, retained pinned versions and self-update. Linux, Windows and macOS are
explicit host selections; a metadata listing alone never proves native execution.
The [dated qualification record](https://github.com/rustcommons/rust-mirror/blob/main/docs/evals/2026-11-11-rustup-lifecycle.md) distinguishes
native/server results and pending gates. Nexus tests use an isolated legacy OSS
fixture, not your production server. See [hosting and limitations](https://github.com/rustcommons/rust-mirror/blob/main/docs/NEXUS-RAW.md).

## Planned products

- **rust-mirror**: core library and CLI, inspired by [Panamax](https://github.com/panamax-rs/panamax).
  Cargo crate/index mirroring remains planned; scheduling is outside 0.1.0.
- **rust-mirror-gui**: future interface over the same core. No GUI implementation
  or framework has been selected.

The repository was renamed from `rustup-offline-mirror` on 2026-11-09, preserving
Git history and stable `ROM-REQ` identifiers. The earlier Windows-local uncommitted
prototype remains untouched and separately preserved; its assessment is historical.

## Documentation

- [Start testing: one-shot sync and demos](https://github.com/rustcommons/rust-mirror/blob/main/docs/SYNC.md).
- [Multi-version generations and explicit channel refresh](https://github.com/rustcommons/rust-mirror/blob/main/docs/MULTI-VERSION.md).
- [Resumable downloads and recovery](https://github.com/rustcommons/rust-mirror/blob/main/docs/RESUMING.md).
- [Toolchain selections, discovery, import and frozen plans](https://github.com/rustcommons/rust-mirror/blob/main/docs/SELECTIONS.md).
- [Nexus Raw hosting and offline installation](https://github.com/rustcommons/rust-mirror/blob/main/docs/NEXUS-RAW.md).
- [Offline proof evidence](https://github.com/rustcommons/rust-mirror/blob/main/docs/evals/2026-11-09-offline-toolchain.md).
- [Recovered prototype assessment](https://github.com/rustcommons/rust-mirror/blob/main/docs/evals/2026-11-09-prototype-assessment.md).
- [Requirements and evidence index](https://github.com/rustcommons/rust-mirror/blob/main/docs/REQUIREMENTS.md).
- [Changelog](https://github.com/rustcommons/rust-mirror/blob/main/CHANGELOG.md) and [release policy/checklist](https://github.com/rustcommons/rust-mirror/blob/main/docs/RELEASING.md).
- [Contribution and validation instructions](https://github.com/rustcommons/rust-mirror/blob/main/AGENTS.md).
- [License](https://github.com/rustcommons/rust-mirror/blob/main/LICENSE).
