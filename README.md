# rust-mirror

[![CI](https://github.com/rustcommons/rust-mirror/actions/workflows/ci.yml/badge.svg)](https://github.com/rustcommons/rust-mirror/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/rustcommons/rust-mirror/badge)](https://scorecard.dev/viewer/?uri=github.com/rustcommons/rust-mirror)
[![crates.io](https://img.shields.io/crates/v/rust-mirror.svg)](https://crates.io/crates/rust-mirror)
[![docs.rs](https://docs.rs/rust-mirror/badge.svg)](https://docs.rs/rust-mirror/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/rustcommons/rust-mirror/blob/main/LICENSE)
[![SLSA Build Level 3](https://slsa.dev/images/gh-badge-level3.svg)](https://github.com/rustcommons/rust-mirror/blob/main/docs/evals/2026-11-16-publication-0.5.4.md#seven-live-files-and-reusable-provenance)

Prepare verified Rust toolchain and curated crate mirrors for offline development.
Publish finished generations to a fresh Nexus Raw or static HTTPS prefix.

## Install

**Download a prebuilt binary** from
[GitHub Releases](https://github.com/rustcommons/rust-mirror/releases/latest).
[Verify checksums/provenance and follow the unsigned-download instructions](https://github.com/rustcommons/rust-mirror/blob/main/docs/INSTALL.md).

| Computer | Archive target |
| --- | --- |
| Windows, Intel/AMD 64-bit | x86_64-pc-windows-msvc |
| Linux glibc 2.17+ (0.5.2+) | x86_64-unknown-linux-gnu |
| Any x86_64 Linux (fully static musl) | x86_64-unknown-linux-musl |
| macOS, Apple Silicon | aarch64-apple-darwin |
| macOS, Intel | x86_64-apple-darwin |

[0.5.5 updates dependencies](https://github.com/rustcommons/rust-mirror/blob/main/docs/releases/v0.5.5.md)
and is the first release whose downloads include the signed provenance bundle
(`.intoto.jsonl`) for offline verification. 0.5.4 first shipped the 0.5.3 security
fixes as downloads, with an attested Cargo SBOM. Version 0.5.3 is crate-only: it has no prebuilt downloads.
0.5.2 adds the GNU download; v0.5.1 keeps its four original archives.
GNU targets glibc 2.17; its container userland checks cover CentOS 7,
AlmaLinux 8, openSUSE Leap 42.3, Ubuntu 16.04/18.04/20.04 and Debian 8/9.
Old kernels are untested; for glibc below 2.17 use static musl. Choose GNU when your
environment prefers dynamic glibc, or musl to avoid shared-library dependencies.
No Rust installation is needed. On Windows the C runtime is built in, so no Visual
C++ Redistributable is needed. See the install guide for runtime and DNS limits.

**Install with Cargo**, using Rust/Cargo 1.90 or newer:

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
| [Crate guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/CRATES.md) | Catalogs, approved previews, sparse registry and Cargo client setup |
| [API docs](https://docs.rs/rust-mirror/latest/rust_mirror/) | Rust library and controlled operations |
| [Corporate networks](https://github.com/rustcommons/rust-mirror/blob/main/docs/NETWORK.md) | Proxy/CA/timeouts, connection diagnostics, size estimates and cached release discovery (0.3.0+) |
| [Events protocol](https://github.com/rustcommons/rust-mirror/blob/main/docs/EVENTS.md) | Loosely coupled CLI/GUI JSON lines, heartbeat and cancellation |
| [Retention](https://github.com/rustcommons/rust-mirror/blob/main/docs/RETENTION.md) | Declared protection, exact previews and opt-in local pruning |
| [Nexus](https://github.com/rustcommons/rust-mirror/blob/main/docs/NEXUS-RAW.md) | TLS/auth, no-write preflight and publication |
| [Requirements](https://github.com/rustcommons/rust-mirror/blob/main/docs/REQUIREMENTS.md) | Bounded verification and remaining gaps |
| [Changelog](https://github.com/rustcommons/rust-mirror/blob/main/CHANGELOG.md) | Release history |

For curated crates, save a [catalog](https://github.com/rustcommons/rust-mirror/blob/main/docs/CRATES.md#catalog-and-paste), then preview and approve:

```sh
rust-mirror crates sync --config crates.toml --output ./crate-run-001
rust-mirror crates show --plan ./crate-run-001/plan.json
# Review the whole preview, then substitute its exact plan_id:
rust-mirror crates sync --config crates.toml --output ./crate-run-001 --approve-plan PLAN_ID
```

0.5.0 also copies named Cargo.lock pin sources into the catalog; preview with
`rust-mirror crates import-lock --config crates.toml --lock service-a=../service-a/Cargo.lock`
and explicitly add `--write` to save. See the [lock guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/LOCK-IMPORT.md).

The experimental crate MVP starts in 0.4.0. Client build evidence is bounded;
historical/current cohort combinations that need unmirrored edges are refused.
Scheduling, in-generation compaction and Nexus pruning remain deferred.
Upstream signatures are preserved, not independently verified. Native SDKs and
production server policy remain administrator responsibilities. See the guides for
trust/platform limits. Inspired by [Panamax](https://github.com/panamax-rs/panamax).
Licensed under [Apache-2.0](https://github.com/rustcommons/rust-mirror/blob/main/LICENSE).
