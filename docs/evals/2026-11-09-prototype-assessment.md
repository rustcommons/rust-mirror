# Recovered prototype assessment - 2026-11-09

## Verdict and scope

Keep the recovered prototype as a preserved reference. Reuse its separation of configuration,
networking and channel metadata, plus its local-HTTP testing pattern. Replace the manifest
selection/rewrite and filesystem publication logic before treating it as a mirror engine.
It is a small, buildable experiment, not an operational offline mirror or Panamax replacement.

This assessment covers the owner's uncommitted Windows prototype, not an implementation
already in GitHub. All 21 original files (75,804 bytes) and the original Git status were
hash-checked unchanged after the assessment. A separate snapshot and ZIP are preserved in
coordinator thread storage. No original files were edited, deleted, imported or committed.
The [inventory](2026-11-09-prototype-inventory.json) records every original path and SHA-256.

## What exists

| Area | Actual behavior | Disposition |
| --- | --- | --- |
| Library layout | Separate configuration, errors, HTTP and channel modules | Retain the separation; make a GUI consume the same core library later. |
| Configuration | TOML loading; nonempty selection lists and positive concurrency validation | Retain the idea; replace permissive schema and validate supported options. |
| HTTP | Blocking text GET, timeout and error translation | Retain local-server test pattern; add bounded streaming, integrity checks and deliberate URL/redirect policy. |
| Manifest processing | Parse TOML v2 header, remove package/target entries, rewrite `url` | Replace selection and URL logic; findings below prevent offline correctness. |
| Filesystem output | Write rewritten manifest and a SHA-256 sidecar | Replace direct writes with checked staging and coherent publication. |
| CLI | Read `mirror-config.toml` from current directory and print settings | Replace with real commands; even `--help` currently prints configuration. |
| Tests | Eight integration tests using tiny fixtures and loopback servers | Keep useful test patterns; substantially expand coverage with real metadata and failure cases. |

The prototype does not download component archives, bootstrap rustup, mirror crates or
indexes, serve content, create/import bundles, verify an offline installation, or implement
resume, pruning, locking or transactional publication. Its README's deterministic bundles,
safe extraction and cryptographic verification are goals, not completed functionality.
There is no extractor whose safety can be assessed. `max_concurrency` is validated but not
used to schedule work. `verify_checksums` is stored/printed but not used to verify upstream
metadata or artifacts. Calculating a checksum for rewritten bytes does not authenticate
those bytes or validate their upstream source.

## Reproduced findings

Seven focused probes were added only to the disposable copy. They intentionally assert the
observed defects; passing these probes means the defects were reproduced, not fixed.

| ID | Finding | Evidence and effect |
| --- | --- | --- |
| P01 | Real manifest loses required entries and retains upstream compression URLs | Official Rust 1.90.0 metadata rewrites successfully with the prototype's sample config, but removes `pkg.rust`, `clippy-preview` and `rustfmt-preview`; `rustc`'s `xz_url` remains on static.rust-lang.org while `url` points to the mirror. This output does not establish a usable offline Rustup channel. |
| P02 | Unavailable targets are treated as malformed | A selected target with `available = false` and no URL produces `target.url missing`. Valid unavailable entries need explicit handling. |
| P03 | Target-independent packages disappear | A selected `rust-src` entry under target `*` is removed by exact-triple filtering. |
| P04 | Unsupported configuration is silently accepted | `version = 999` and `verify_signatures = true` parse successfully. The signature setting has no field/implementation; unknown keys are ignored. A requested security property must be implemented or rejected. |
| P05 | URL rewriting uses string-prefix matching | `https://static.rust-lang.org.evil.invalid/dist/...` becomes a malformed mirror-origin string rather than being recognized as a different origin. Parse URLs and define allowed sources; do not use hostname text prefixes. |
| P06 | Component references can dangle | A retained `rust` entry can reference `rustc` after that package was filtered out. Selection must include metadata dependency closure, profiles, component aliases and extensions consistently. |
| P07 | Channel text can escape the configured output directory | Calling the public pipeline with `stable/../../../escaped`, with a prepared intermediate directory, writes manifest and sidecar outside `output_dir`. Demonstrated wholly inside a temporary sandbox. The current CLI does not invoke this function, but it must be fixed before exposure. |

P01-P06 are manifest/configuration correctness and trust-boundary defects. P07 is a filesystem
containment defect. Additional source-observed gaps, not independently fault-injected:
manifest and checksum use separate direct writes (interruption/concurrency can expose an
inconsistent pair); there is no explicit application-level byte budget; original upstream
checksums/signatures are not fetched/verified; declared hashes are not validated; and there
is no artifact completion check before a channel could be exposed.

One existing test evaluates `matches!(...)` without asserting the result. Its error-kind
expectation therefore cannot fail the test. Minimal fixtures omit compression variants,
real profiles/aliases, unavailable targets, wildcard packages and installation closure.

## Executed validation

Environment: Windows x86-64, Rust/Cargo 1.99.0, existing installed MSVC toolchain.
Original source and lock were tested in a copied directory. Missing locked dependencies
were downloaded after an initial offline attempt failed on an uncached dependency; no
project dependencies were added, versions updated or tools installed.

| Check | Result |
| --- | --- |
| `cargo test --locked --all-targets -- --test-threads=2` | Original suite: 8 integration passes; 0 library/binary unit tests. |
| `cargo test --locked --offline --test assessment_probes -- --test-threads=2` | All 7 documented defects reproduced. |
| `cargo test --locked --offline --doc` | Pass, but 0 doctests. |
| `cargo fmt -- --check` | Fails; formatting changes needed. No formatting applied. |
| `cargo clippy --locked --offline --all-targets -- -D warnings` | Fails on `AppConfig::from_str` / `should_implement_trait`; not a correctness proof or exhaustive lint inventory. |
| Binary `--help` | Exit 0, prints configuration; confirms absent CLI argument handling. |
| Preservation check | All 21 source snapshot hashes and original Git status unchanged. |

No full toolchain/artifact downloads, offline Rustup installations, Cargo crate builds,
Linux/macOS qualification, deployment tests, dependency security audit or benchmarks were
performed. These checks do not qualify a release.

## Panamax comparison and proposed development order

Panamax documents toolchain and crate mirroring, rustup bootstrap, project-selected crate
sync and an HTTP server. None of those end-to-end workflows exists in this prototype.
See its [pinned README](https://github.com/panamax-rs/panamax/blob/84ca2d7064f5a9931371f9eb74c4003707897a7b/README.md).
Rustup's [manifest model](https://github.com/rust-lang/rustup/blob/baa72a555a44bcede1a602454531604af80ebba9/src/dist/manifest.rs)
models wildcard targets, compression alternatives, availability, component relationships,
profiles and renames; the prototype handles only a subset.

Recommended milestones below are proposals, not an approved detailed feature contract:

1. **Core foundation:** separate the reusable library from a thin `rust-mirror` CLI; retain
   the original snapshot, add strict versioned configuration, typed source/path validation,
   real fixture coverage and CI. Expose progress/cancellation through the core so a future
   GUI does not need a second mirroring implementation. No GUI framework decision yet.
2. **One complete Rustup workflow:** pinned release, explicit host/components/targets,
   dependency-aware artifact plan, bounded downloads, verified hashes, atomic staging and
   an explicit provenance policy for any derived metadata. Include rustup bootstrap and
   prove installation from the mirror with upstream access unavailable. Expand channels
   and historical releases only after the pinned path works.
3. **Cargo mirroring:** start with lockfile-selected registry dependencies and sparse index
   serving. Verify archives against index checksums; test Cargo source replacement and a
   real build without upstream access. Specify git dependencies, build-script downloads,
   alternate registries and native prerequisites as separate closure questions.
4. **Operational reliability:** interrupted-download recovery, concurrent writers, safe
   pruning, transfer manifests and verified export/import. Make the same snapshot usable
   through the built-in server or an external static host. Test crash and corruption cases.
5. **rust-mirror-gui:** build on the tested core and its progress/cancellation interfaces;
   choose UI technology and packaging later.

Do not import this prototype unchanged and describe it as a production mirror. Its best
value is a reference implementation and a set of starting tests. No release or crate-name
reservation is part of this assessment.
