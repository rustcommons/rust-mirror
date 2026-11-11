# rust-mirror

Mirror selected Rust toolchains for offline installation and updates using Rustup.
Host the verified files on Nexus Repository Raw or another static HTTP server.

**[Getting started and API docs](https://docs.rs/rust-mirror/latest/rust_mirror/)** |
**[Complete user guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/USER-GUIDE.md)** |
**[Command reference](https://github.com/rustcommons/rust-mirror/blob/main/docs/CLI-REFERENCE.md)**

## What it does

- Select compiler versions or dated channels, host platforms, profiles, components
  and compilation targets.
- Plan downloads before transferring archives; verify hashes and resume interrupted
  transfers from frozen metadata.
- Build immutable generations, retain older toolchains and provide Rustup bootstrap,
  self-update and numeric-version lookup endpoints.
- Publish verified files to a fresh Nexus Raw prefix over HTTPS, including private-CA
  and scoped Basic-authentication support.

Rustup installs and updates client tools. rust-mirror prepares and publishes their
files; it does not replace Rustup, run an HTTP server or activate Nexus routing.

## Install

Rust/Cargo **1.90 or newer** is required to build:

```sh
cargo install rust-mirror --version 0.1.1 --locked
rust-mirror --help
```

The installed CLI has no Python dependency. Python is used only by optional
repository helpers and qualification scripts.

## Prepare your first mirror

On an internet-connected machine, save this as `mirror.toml`:

```toml
schema = 1
rustup_version = "1.29.3"
max_download_bytes = 10737418240

[[selections]]
name = "linux-team"
toolchain = "1.90.0"
hosts = ["x86_64-unknown-linux-gnu"]
profile = "default"
components = []
targets = []
```

A **host** is where the compiler runs. A **target** is where the resulting program
runs. For example, use `x86_64-pc-windows-msvc` for Windows developers or add
`wasm32-unknown-unknown` to `targets` for WebAssembly output. The Rustup manager
version is separate from the Rust compiler version. Availability depends on the
selected upstream release; native qualification is documented separately.

```sh
# Inspect availability without downloading compiler archives.
rust-mirror list --toolchain 1.90.0 --host x86_64-unknown-linux-gnu

# Resolve every selection, download and verify its files, then assemble a generation.
rust-mirror sync --config mirror.toml --output ./run-001

# Complete the Rustup bootstrap, update and numeric-version endpoints.
rust-mirror rustup --snapshot ./run-001/mirror --version 1.29.3 --output ./generation-001
```

Use new output directories whose parents exist. The completed tree to transfer or
host is **`generation-001/mirror`**. Private working directories are not served.
For separate review/download steps, use `plan`, `fetch` and `assemble` instead of
`sync`. See the [docs.rs walkthrough](https://docs.rs/rust-mirror/latest/rust_mirror/)
for commands and a library example.

## Publish and use it

Provision a fresh Nexus Raw hosted repository or prefix with read/upload permissions
and redeployment disabled. Then upload the completed generation:

```sh
rust-mirror publish --source ./generation-001/mirror --base-url https://nexus.example.local/repository/rust-generation-001
```

For a private CA, add `--ca-file /path/to/company-ca.pem`. Set both
`NEXUS_USERNAME` and `NEXUS_PASSWORD` through your secret-management process when
Basic authentication is required; do not put credentials in the URL.

Publication checks that every destination is absent, uploads and GET-verifies the
bytes, and writes the inventory last. It does not configure Nexus or activate a
Raw group. An operator exposes the complete generation only after verification.
[The Nexus guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/NEXUS-RAW.md)
explains routing, permissions, trust and partial-upload recovery.

On a client that already has Rustup installed, point it at the activated root:

```sh
# Bash/sh; use your approved internal server URL.
export RUSTUP_DIST_SERVER=https://nexus.example.local/repository/rust-active/dist
export RUSTUP_UPDATE_ROOT=https://nexus.example.local/repository/rust-active/rustup
rustup toolchain install 1.90.0 --profile default --no-self-update
```

The [user guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/USER-GUIDE.md)
has fresh-client bootstrap, Windows PowerShell, private-home and existing-client
update examples. Client CA trust is separate from the publisher's `--ca-file`.

## Update and recover

| Need | Use |
| --- | --- |
| Continue an interrupted run | `sync --resume` or `fetch --resume` with identical frozen inputs |
| Restart managed sync output | `sync --force`; destroys its old managed data **without a backup** |
| Retain history in the next generation | New output plus `sync --previous PREVIOUS_MIRROR` |
| Review a project's toolchain requirements | `import` a `rust-toolchain.toml`, then inspect/plan |
| Recover a partial upload | A fresh remote prefix; publishing has no resume or overwrite option |

Read [sync](https://github.com/rustcommons/rust-mirror/blob/main/docs/SYNC.md),
[resume](https://github.com/rustcommons/rust-mirror/blob/main/docs/RESUMING.md) and
[retained generations](https://github.com/rustcommons/rust-mirror/blob/main/docs/MULTI-VERSION.md)
for the exact rules. Publishing and client activation remain separate operations.

## Scope and trust

Current releases cover selected modern Rustup distributions. Native client evidence
covers GNU Linux x86-64, Windows MSVC x86-64, Intel macOS and Apple Silicon macOS at
the recorded versions. Metadata availability is not proof that another host works.
Nexus qualification used an isolated legacy OSS fixture, not your production server.

Upstream manifest/signature bytes are preserved. Downloads are checked against
upstream hashes over HTTPS; independent OpenPGP validation is **not** implemented.
OS SDKs/linkers, Cargo crate/index mirroring, scheduling, pruning and the planned
`rust-mirror-gui` are outside this release. There is no globally atomic activation
or automatic retry guarantee.

## Unreleased local retention

The source checkout adds the planned 0.2.0 local `prune` feature, with declared
pins/stable/nightly/rollback protection, local generation/run discovery,
next-previous protection, exact structured previews and explicit
`--apply`. Published 0.1.x binaries do not include it. It removes whole generations;
kept collections are not compacted and Nexus deletion remains deferred. See the
[retention guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/RETENTION.md).

## Documentation

| Read | For |
| --- | --- |
| [docs.rs](https://docs.rs/rust-mirror/latest/rust_mirror/) | Usage walkthrough, library examples and API reference |
| [User guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/USER-GUIDE.md) | Complete Windows/Unix preparation, hosting and update workflow |
| [CLI reference](https://github.com/rustcommons/rust-mirror/blob/main/docs/CLI-REFERENCE.md) | Every command, option, default, report and limit |
| [Rustup lifecycle](https://github.com/rustcommons/rust-mirror/blob/main/docs/RUSTUP.md) | Bootstrap, self-update, aliases and promotion |
| [Nexus hosting](https://github.com/rustcommons/rust-mirror/blob/main/docs/NEXUS-RAW.md) | Server roles, publication and trust |
| [Changelog](https://github.com/rustcommons/rust-mirror/blob/main/CHANGELOG.md) | Release changes |
| [Qualification](https://github.com/rustcommons/rust-mirror/blob/main/docs/evals/2026-11-11-rustup-lifecycle.md) | Exact tested versions, outcomes and remaining gaps |

Inspired by [Panamax](https://github.com/panamax-rs/panamax). Licensed under
[Apache-2.0](https://github.com/rustcommons/rust-mirror/blob/main/LICENSE).
