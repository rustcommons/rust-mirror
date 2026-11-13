# Native crate resolution Phase 0 — 2026-11-13

## Scope and source

Owner decisions dated 2026-11-12 authorized native Phase 0 with declared features,
one whole-preview approval per plan, direct `semver` and previously approved
`serde_json`. Runtime external/downloaded Cargo, the Cargo library and unapproved
solver crates are excluded. Follow the [design](../CRATES-DESIGN.md) and
[roadmap](../ROADMAP.md). This record concerns research APIs, not a production
registry generation, fetch/publish implementation, version assignment or release.

Implementation is [src/crates](../../src/crates/mod.rs); the
[research driver](../../examples/crates_phase0.rs) accepts exact roots and a private
metadata cache. It is not the future Cargo-compatible catalog parser. The native
API can only request index records, uses existing explicit NetworkOptions and returns
serializable records/reasons/witnesses. It does not invoke Cargo, fetch archives or
extract crates. New package dependencies are the approved semver/serde_json only
(with their resolved transitive dependencies). Existing toolchain origin rules are
unchanged; only this separate metadata source uses index.crates.io.

## Local corpus observation

Rust/Cargo 1.90.0 on Linux. Exact roots, metadata hashes, plans, full/filtered Cargo
locks and logs are emitted by [the oracle](../../scripts/check_crates_phase0.py).
The local corpus used 179 sparse files / 17,952,481 bytes, with 1,042 native search
attempts. Every root's filtered-index `cargo generate-lockfile` succeeded with zero
archive requests. The full-index Cargo reference uses the same cached observations;
additional metadata is allowed only in that test-reference pass. This is not a
claim about arbitrary projects or future Cargo versions.

| Exact root / enabled features | Native versions | Full-index Cargo versions | Native-only versions |
| --- | ---: | ---: | ---: |
| tokio 1.48.0 / full | 24 | 24 | 0 |
| serde 1.0.228 / derive | 9 | 7 | 2 |
| clap 4.5.50 / derive | 21 | 21 | 0 |
| reqwest 0.12.24 / default | 155 | 155 | 0 |
| rand 0.8.5 / default | 14 | 14 | 0 |
| rand 0.9.2 / default | 16 | 16 | 0 |
| openssl-sys 0.9.110 / default | 7 | 7 | 0 |
| syn 2.0.110 / full, visit, fold, extra-traits | 4 | 4 | 0 |

Deduplicated native and Cargo lockfile unions both contain **195** versions:
intersection 195, native-only 0, Cargo-only 0, union version overfetch **0%**.
Serde alone has two conservative extras (`serde_derive` 1.0.229 and `syn` 3.0.10),
**28.57%** over its seven-version Cargo solution; those versions are already needed
elsewhere in the corpus union. These are version-count comparisons, not byte
savings. Archive byte overfetch is unknown: no HEAD sizes or actual upstream
archives were fetched for this planning exercise. Sparse metadata necessarily
contains upstream records for versions not selected; it is not published output.

## Counterexamples and regression gates

Newest-per-edge alone failed the feature-intersection and prerelease-intersection
proof; native bounded search supplies the older joint candidate. Tests cover exact
pins, parent backtracking under native `links` conflicts, prerelease/caret/tilde/
wildcard/comparison requirements, yanks, default-feature choices, aliases, dep:/weak
forwarding, schema 2/features2, all-platform build edges, dev exclusion, byte
preservation, budgets, source/schema refusal and typed cancellation.

The initial corpus probe passed tokio/serde/clap but failed reqwest/default:
filtered output omitted hyper-rustls and Cargo refused resolution. A minimal local
Cargo 1.90 fixture reproduced [Cargo issue 10801](https://github.com/rust-lang/cargo/issues/10801):
enabled weak forwarding causes otherwise inactive optional dependencies to appear
in Cargo.lock. The corrected native graph records this as `lock_only` coverage,
without claiming build activation. A disabled-forwarding fixture excludes it; a
cheap build fixture deliberately fails if that inactive dependency is compiled.
Preserve this initial failure as evidence of why simple build-feature closure is
insufficient for a filtered sparse mirror.

[Independent Cargo fixtures](../../scripts/crates_phase0_fixtures.py) compare native
and real-Cargo success/refusal, then fetch/build successful tiny fixtures. These
archives are generated locally with Python's standard library; no Rust tar/flate2
dependency or upstream payload is needed. Archive requests are forbidden during
planning/lock generation and enabled only for the separate fixture fetch step.

## Validation and remaining limits

Nine focused Rust cases and local popular-crate/synthetic Cargo conformance passed.
The full Rust suite (including doctest), warning-denied all-target clippy, warning-
denied rustdoc, 24 Python tests, source/link/render checks and clean diff passed.
The first full-suite compile exposed serde_json's additional PartialEq implementation
making an existing retention-test sum ambiguous; adding `sum::<u64>()` fixes inference
without changing retention behavior. Exact-head CI is still required for Phase 0 exit.
The [conformance workflow](../../.github/workflows/crates-phase0.yml) retains exact
metadata, plans, hashes, locks and logs, including on failure. An oracle mismatch
blocks release; a native witness is not a formal equivalence proof for every Cargo
resolver or Rust compiler.

Bounds cover index count/bytes, semantic record size, candidate/graph work, versions
and search attempts. Search uses explicit frames, not recursive dependency depth;
filtered replay cannot fetch new index files. Cancellation is cooperative and socket
I/O retains the existing timeout/DNS limits. Phase 0 does not claim persistent frozen
fetch state, whole-catalog latest-N ingestion, retained generations, authenticated
Cargo clients, license approval, payload checksums, GUI integration, publication,
remote deletion, resolver-3/MSRV policy or native Windows client qualification.
Cargo.lock import remains deferred. No new release/publication is authorized.
