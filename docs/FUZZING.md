# Parser fuzzing

The independent [fuzz workspace](../fuzz/Cargo.toml) is unpublished and excluded
from the distributed crate by the root package's explicit include list. The shipped
crate's manifest, lock and dependency graph are unchanged. The owner approved
libfuzzer-sys and its arbitrary dependency for fuzzing only on 2026-11-17.

| Target | Production entry points / scope | Small seed sources |
| --- | --- | --- |
| lockfile | lockfile::parse_with_control, standard lock v1–4, metadata checksums, external skips | tests/crates_lockfile.rs format fixtures |
| catalog | Catalog::parse, Catalog::from_dependencies, JSON Catalog + validate | tests/crates_catalog.rs normalization and rejection cases |
| sparse_index | native crates::plan, real line/record, feature and semver validation | tests/crates_phase0.rs synthetic record shape; fixed fixture root |
| channel_manifest | selection::catalog through an in-memory Source, hash-correct arbitrary manifests | tests/common/mod.rs minimal fixture host/date/packages |
| events | Event JSON-lines parsing and canonical JsonLines output; one newline and valid JSON assertions | tests/events.rs protocol hello/bytes/result shapes |
| network | TOML/JSON NetworkConfig::validate, proxy URL and exclusion syntax | tests/network.rs direct/manual proxy shapes |
| inventory | publication's Snapshot/SelectionSnapshot/CollectionSnapshot deserialization, actual crate Inventory::load | tests/publish.rs legacy inventory shape and incomplete schema-5 refusal |

Seeds are small synthetic variants of those tests, not copyrighted archive payloads
or credentials. The in-memory index source refuses other crate names; the channel
source supplies the matching checksum/signature and refuses all other requests.
Targets neither access upstream nor resolve environment credentials. Inventory
checks use a disposable temporary directory, removed on every iteration. No target
publishes, runs external Cargo, or writes to user homes. Invalid input is normally a
returned error; panics, sanitizer findings and framing failures fail the run.

Inputs are capped at 64 KiB; index resolution also bounds crates/versions/work/
backtracking, and event streams at 32 lines. Larger production limits, multi-crate
resolution completeness, network behavior and full publication are covered by other
tests, not inferred from these adapters. Legacy inventory fuzzing exercises its
actual deserialization types; only crate inventory fuzzing invokes full inventory
validation. Fuzzing does not authenticate metadata or test the upstream registries.

## Run locally

Use the product MSRV for ordinary tests. Fuzz instrumentation uses the frozen
nightly-2025-09-05 (rustc 1.91.0-nightly, af00ff2ce) and cargo-fuzz 0.13.1:

```sh
rustup toolchain install nightly-2025-09-05 --profile minimal --component rust-src
cargo install cargo-fuzz --version =0.13.1 --locked
cargo fetch --locked --manifest-path fuzz/Cargo.toml
CARGO_NET_OFFLINE=true cargo +nightly-2025-09-05 fuzz run lockfile fuzz/corpus/lockfile -- -max_total_time=60 -max_len=65536 -timeout=10 -rss_limit_mb=2048
```

A C/C++ compiler is required for libFuzzer (Clang recommended). cargo-fuzz 0.13.1
has no --locked build/run flag: fetch with --locked first, run offline, and inspect
that fuzz/Cargo.lock remains byte-identical. The CI build enforces that hash check.
Repeat the final command for any target above; keep minimized findings and add a
normal deterministic regression before fixing a bug. Do not catch panics or ignore
crashes to make CI pass.

## Continuous jobs and limits

[PR fuzzing](../.github/workflows/cflite_pr.yml) uses ordinary pull_request, never
pull_request_target, and code-change mode for 300 seconds on src/fuzz changes (plus
its own configuration and dependency locks). [Weekly batch](../.github/workflows/cflite_batch.yml)
uses 1800 seconds; both allow manual dispatch and skip Dependabot. These are total
engine budgets shared among targets, not 300/1800 seconds for each target.

[The builder](../.clusterfuzzlite/Dockerfile) is digest-pinned, contains the dated
nightly/cargo-fuzz, and builds with address sanitizer through
[build.sh](../.clusterfuzzlite/build.sh). The script freezes the fuzz lock, includes
per-target options and zipped seeds, and exports all seven executables. Upstream
ClusterFuzzLite engine actions are SHA-pinned; their internal runtime images remain
managed by that upstream action, a transitive supply-chain limit. Crash artifacts
are retained by ClusterFuzzLite. SARIF is uploaded from cifuzz-sarif/results.sarif
when produced; fork PRs do not receive security-events writes. No external corpus
repository, write token or persistent corpus service is configured: batch corpora
remain run artifacts, and reviewed small seeds live in Git.

References: [official Rust integration](https://google.github.io/clusterfuzzlite/build-integration/rust-lang/),
[GitHub Actions modes](https://google.github.io/clusterfuzzlite/running-clusterfuzzlite/github_actions/),
[Scorecard check definitions](https://github.com/ossf/scorecard/blob/61e84743356275f6ffca23b1b2f2cb945e19eb96/docs/checks.md).
