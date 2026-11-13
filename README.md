# rust-mirror

Prepare verified Rust toolchain mirrors for offline Rustup installation and updates.
Publish finished generations to a fresh Nexus Raw or static HTTPS prefix.

## Install

Rust/Cargo 1.90 or newer:

```sh
cargo install rust-mirror --locked
```

## Quick start

```sh
rust-mirror init --config mirror.toml
# Edit explicit versions, hosts and components in mirror.toml.
rust-mirror sync --config mirror.toml --output ./run-001
rust-mirror rustup --snapshot ./run-001/mirror --version 1.29.3 --output ./generation-001
```

Output directories must be new. Host or transfer `generation-001/mirror`; private
working directories are not served. Publication uses an explicitly selected fresh
prefix and does not activate Nexus routing. Start with a disposable local setup.

| Guide | Contents |
| --- | --- |
| [User guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/USER-GUIDE.md) | Full Windows/Unix workflow, prerequisites, hosting and recovery |
| [Command reference](https://github.com/rustcommons/rust-mirror/blob/main/docs/CLI-REFERENCE.md) | Commands, configuration, effects and limits |
| [API docs](https://docs.rs/rust-mirror/latest/rust_mirror/) | Rust library and controlled operations |
| [Corporate networks](https://github.com/rustcommons/rust-mirror/blob/main/docs/NETWORK.md) | Proxy/CA/timeouts, connection diagnostics, size estimates and cached release discovery (0.3.0+) |
| [Events protocol](https://github.com/rustcommons/rust-mirror/blob/main/docs/EVENTS.md) | Loosely coupled CLI/GUI JSON lines, heartbeat and cancellation |
| [Retention](https://github.com/rustcommons/rust-mirror/blob/main/docs/RETENTION.md) | Declared protection, exact previews and opt-in local pruning |
| [Nexus](https://github.com/rustcommons/rust-mirror/blob/main/docs/NEXUS-RAW.md) | TLS/auth, no-write preflight and publication |
| [Requirements](https://github.com/rustcommons/rust-mirror/blob/main/docs/REQUIREMENTS.md) | Bounded verification and remaining gaps |
| [Changelog](https://github.com/rustcommons/rust-mirror/blob/main/CHANGELOG.md) | Release history |

The [crate MVP](https://github.com/rustcommons/rust-mirror/blob/main/docs/CRATES.md)
is implemented only in development source; published 0.3.0 has no crate registry
mirroring. Scheduling, in-generation compaction and Nexus pruning remain deferred.
Upstream signatures are preserved, not independently verified. Native SDKs and
production server policy remain administrator responsibilities. See the guides for
trust/platform limits. Inspired by [Panamax](https://github.com/panamax-rs/panamax).
Licensed under [Apache-2.0](https://github.com/rustcommons/rust-mirror/blob/main/LICENSE).
