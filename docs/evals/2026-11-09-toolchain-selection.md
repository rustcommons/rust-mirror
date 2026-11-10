# Toolchain selection iteration: 2026-11-09/10 UTC

The owner requested Rustup completion first, with selectable tooling versions and
platforms. This iteration adds saved selections, availability discovery, project
file import, metadata-only planning and frozen downloads. It is not a release or
a claim that all Rustup workflows are complete. Work is stacked on the open Nexus
integration PR; the original recovered prototype is unchanged.

## Source and reference boundary

[Source-file identities](toolchain-selection-source.json) cover the tested Rust,
Python, fixtures, example and unchanged Cargo manifest/lock. Inventory SHA-256:
`ce57d17ca4b7229c35a6a79745468860ebe4a0da25e61bc44b011b20fae6ac7a`.
The final source commit and hosted results are recorded in the PR. There are no
new dependencies, version changes, tags, crate publication or GUI work.

Panamax was inspected at
[`84ca2d7064f5a9931371f9eb74c4003707897a7b`](https://github.com/panamax-rs/panamax/tree/84ca2d7064f5a9931371f9eb74c4003707897a7b).
Its [default configuration](https://github.com/panamax-rs/panamax/blob/84ca2d7064f5a9931371f9eb74c4003707897a7b/src/mirror.default.toml)
provides platform filters, pinned versions, archive-format controls and retention
settings. The [Rustup implementation](https://github.com/panamax-rs/panamax/blob/84ca2d7064f5a9931371f9eb74c4003707897a7b/src/rustup.rs)
is a reference for synchronization concepts. No Panamax source was copied, no
maintainer was contacted, and this is not an audit of Panamax.

| Concern | This iteration |
| --- | --- |
| Platform/version selection | Named selections; explicit hosts; per-release metadata |
| Required tools | Minimal/default profile plus explicit components and targets |
| Reproducibility | Frozen cached metadata, resolved/date-pinned selector, SHA-256 inventory |
| Archive formats | All advertised variants for the selected closure |
| Continuous synchronization/retention | Still pending; no deletion or overwrite |
| Crate mirroring/server UI | Still pending; Rustup remains first |

## Real metadata matrix

[All 22 cases](toolchain-selection-matrix.json) gave the expected results using the
Windows-built CLI and official HTTPS metadata. No payload archives were fetched
for these plans.

- Twelve Rust 1.90.0 cases: minimal/default on GNU Linux x86_64 and aarch64,
  Windows MSVC and GNU x86_64, and Intel/Apple Silicon macOS.
- A six-host minimal selection; one extended Linux selection with Clippy, rustfmt,
  rust-src, rust-analyzer and Wasm target; one musl Linux selection.
- Moving stable/beta/nightly and dated `nightly-2025-09-18`. Recorded resolutions
  are in the matrix, not promises about future channel contents.
- Explicit unavailable Miri, unknown host and unknown target each refused.

Rust 1.90.0 GNU catalog output recorded 37 hosts, 16 host/global components and
139 compilation targets. This is availability evidence only. Bootstrap checksum
sidecars were checked by planning; foreign installers were not executed.

An earlier WSL-driven matrix stopped on WSL service error `0x8007274c` after two
plans; that failed harness record is retained separately. The complete matrix
was then run with the native Windows binary. No source workaround was required.

## Frozen import/download and isolated execution

Imported an actual `rust-toolchain.toml` selecting Rust 1.90.0, minimal profile,
Clippy, rustfmt, rust-src and `wasm32-unknown-unknown`, with explicit GNU x86_64
host and Rustup 1.28.2. Windows generated the frozen plan; the Linux Rust 1.90
binary fetched that same plan without resolving live channel metadata.

The snapshot contains **15 payload files / 23 inventory files**, including both
advertised gzip/xz formats, original metadata and dated lookup aliases. Recorded
file bytes total **420,519,707** (generated inventory/storage overhead excluded).
Snapshot inventory SHA-256:
`3f0a02543c2eadc69b2e8c973ecd7508bb48706f05cbe1f70f8198821bd80bdf`.

[Final proof summary](toolchain-selection-summary.json) records:

- A new network namespace with only loopback, no route, and external connection
  refusal; new homes for each positive/negative client.
- Verified local HTTP PUT/GET publication, fresh bootstrap and selected install.
- Rust/Cargo/Rustup versions, installed components/targets, offline build/run,
  formatting and Clippy all passed.
- Wasm standard-library metadata typechecking passed; no Wasm link or execution claim.
- Withholding both formats of the actually requested Cargo archive failed with
  404; corrupting both failed checksum verification. Fresh homes prevented reuse.

The first extended proof passed. The final driver then narrowed negative-control
selection to **client** downloads (excluding publisher verification GETs), corrected
summary boundaries, and passed again against unchanged snapshot bytes/new homes.
This prevents a multi-host proof from selecting an archive only the publisher read.
No new actual Nexus qualification follows from this local HTTP fixture.

## Portable checks

- Windows Rust 1.99: 27 Rust tests across the full run and final CLI delta; fmt,
  Clippy and warnings-denied rustdoc passed. Python ran 11 tests with one
  symlink-privilege skip.
- Linux Rust 1.90: 28 Rust tests including Unix link/FIFO refusal and final CLI
  delta; Clippy and warnings-denied rustdoc passed. All 11 Python tests passed.
- The initial system Rust 1.90 Clippy command reported that its component was not
  installed. Checks resumed with the **already installed isolated proof's** Rust
  1.90/Clippy toolchain; no global component installation or skipped check.
- All 21 recovered prototype file hashes and its original Git status are unchanged.

Raw logs, source inventory and native proof packet are retained outside the repo:
`rust-mirror-selection-proof-20261109/toolchain-selections-proof.tar.gz`, 58,525 bytes,
SHA-256 `7b49938319bb40613704d26076e23477422709b5b40d33cf1ab6fe00b2027092`.
The packet excludes toolchain binaries, downloaded archives and private state.

## Remaining work

The [maintained guide](../SELECTIONS.md#limits-and-trust) defines limits. Independent
signature verification, resumable synchronization, shared-cache concurrency,
multiple-version aggregation/retention, Rustup self-update hosting and native
Windows/macOS/musl/ARM install qualification remain pending. Existing native SDKs,
loaders and linkers are prerequisites. Current supported/deployed Nexus onboarding
and schema 2 actual-server qualification remain separate. No EULA was accepted.
