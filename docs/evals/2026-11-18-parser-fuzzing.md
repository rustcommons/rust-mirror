# Parser fuzzing and install pins — 2026-11-18

Owner approval: 2026-11-17. Base main f27b7fb6f263a20d6f658aecd06196b135864609
had green [CI 43869865721](https://github.com/rustcommons/rust-mirror/actions/runs/43869865721).
This change publishes no software and changes no shipped source/dependency/version.

## Source audit and decisions

No active CI-skip convention existed in AGENTS.md, RELEASING.md, workflows or
scripts. Both guides now explicitly prohibit skip markers in commit/merge messages;
CodeQL default setup remains provider-managed. Historical records are unchanged.

Changes beyond existing full-SHA action pins:

- Cargo tool installs use =0.7.0 (inspected cargo-libc source), =0.5.9
  (cargo-cyclonedx), =0.20.2 (cargo-deny); package/client installs use the exact
  manifest or validated release version with an equals requirement and --locked.
- GNU jobs no longer apt-install floating Clang/GCC/binutils: they check and record
  preinstalled tools. Musl uses exact Jammy 1.2.2-4 compiler/runtime/dev packages.
- All ten legacy experiment userlands use inspected Docker registry digests;
  eight already match production's fixed release gates. Alma8 baseline requested
  packages use explicit NEVRAs selected from official Alma8 metadata. The historical
  experiment source checkout is the exact v0.5.1 merge, not a mutable tag spelling.
- There are no workflow pip installs or new download/run installers. The fuzz
  builder's existing toolchain/tools are bound by its Docker manifest digest,
  with nightly-2025-09-05 required explicitly. No curl-to-shell step was added.
- Seven unpublished cargo-fuzz targets, bounded offline adapters, reviewed small
  synthetic seeds and per-target limits are described in [FUZZING.md](../FUZZING.md).
  Actions use full v1 commits; PR code-change budget is 300 seconds, weekly batch
  is 1800 seconds. Forks lack SARIF upload authority; crashes still fail fuzzing.

## Local Scorecard evidence

Official Scorecard 5.6.0, source 61e84743356275f6ffca23b1b2f2cb945e19eb96,
Linux/aarch64 archive SHA-256
3eacaceb3fdee6d0ba5df6ede45d1ada73321949029921bb63633802e8ec5951
was checked against its published checksum before execution. Running
`scorecard --local=DIR --checks=Pinned-Dependencies,Fuzzing --show-details --format=json`
on the base's git archive gave Fuzzing 0, Pinned-Dependencies 10. On this working
source it gave Fuzzing 10, Pinned-Dependencies 10. This is source-pattern detection,
not successful fuzz execution or a new published org score. Scorecard reports
possibly incomplete results for the dynamic CI OS matrix; manual install audit
covers those job snippets. Local SAST cannot establish provider CodeQL history.

## Validation and bounds

All seven fuzz binaries passed stable-MSRV cargo check with the separate lock.
Sanitizer execution, complete local checks and exact-head CI outcomes are recorded
in the PR review record after they finish; this source-audit observation does not
claim those pending outcomes. The root Cargo.toml/Cargo.lock/source and package
include rules are unchanged; fuzz dependencies do not enter the shipped graph.
No sanitizer failure is waived, retried away or converted to an ignored test.

Fuzz targets cap inputs at 64 KiB and use only fixture sources; larger production
inputs and arbitrary network/server behavior are outside this evidence. Legacy
inventory adapters exercise actual parser types; only the crate reader performs
full semantic/payload validation. Distribution package dependencies, hosted OS
images and SHA-pinned actions' internal runtime downloads are not fully hermetic.
ClusterFuzzLite's upstream actions internally name their engine containers by v1;
this repository pins their action source and its own builder digest, not upstream
registry tag mutability. No new service token or external corpus repository is used.
