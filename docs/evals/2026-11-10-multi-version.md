# Multi-version generation iteration: 2026-11-10 UTC

The owner authorized merging resume PR9 and continuing Rustup-first development.
PR9 merged normally at `944bfb719612da91bfcd8046545ff61d6d837d1a`, tree-identical
to reviewed `4c565338`; post-merge Linux/Windows CI42756598927 succeeded.
This iteration implements **local immutable generation assembly**, a step toward
multi-version synchronization. It does not implement automatic fetching/scheduling,
pruning, a mutable remote mirror or a release.

## Contract and reference boundary

The [maintained guide](../MULTI-VERSION.md) defines all commands and limits.
Frozen selected snapshots are assembled with an optional previous generation.
Prior paths/records are retained; only explicit coherent stable/beta/nightly root
metadata may replace the previous root alias. Incoming disagreements and immutable
path conflicts refuse. No input order or version comparison silently picks a winner.
No old-generation files, tags, server state or user homes are deleted.

Panamax's pinned [configuration](https://github.com/panamax-rs/panamax/blob/84ca2d7064f5a9931371f9eb74c4003707897a7b/src/mirror.default.toml)
provides a reference for pinned versions/channels and retained history. This
iteration deliberately exposes frozen-generation composition without implementing
its broader synchronization/retention feature set. No source was copied, no
maintainer contacted, and no Panamax parity/security claim is made.

## Local controls

Permanent collection tests cover:

- Two versions and a later third release, duplicate inputs and retained source bytes.
- Explicit stable root replacement while dated old metadata stays available.
- Conflicting new aliases and changed immutable retained paths, before output creation.
- Final-byte limits, existing output preservation and bootstrap-version disagreement.
- Unexpected/damaged source files and missing selected payloads, checked against manifests.
- Beta/nightly/date-pinned record identity; Unix source symbolic-link refusal.
- Output nested inside a source/previous snapshot refused without creating that output.

Schema 3 publisher preflight and strict CLI singleton/repeated-input parsing are
covered too. Source inventories remain exact path/hash/type checks; assembly also
reconstructs each selected artifact closure from preserved metadata. Signature
bytes are retained, not independently authenticated.

Windows full-suite validation passed, followed by the narrow output-containment
guard tests: **47 distinct Rust tests** in total. Linux Rust 1.90 likewise passed
**50** across its full suite and final guard delta. Clippy, warnings-denied docs,
format checks and 12 Python tests passed; Windows retains one local symlink-privilege
skip. The first Clippy attempt found three unnecessary test-only clones; they were
corrected before the successful full run. Failed and final logs remain separate.
No dependency, version, lock, MSRV or license change.

## Actual two-release mirror

The existing verified GNU Rust 1.90.0 selection was reused, including Clippy,
rustfmt, rust-src and Wasm. A fresh frozen minimal GNU Rust 1.89.0 plan was resolved
from official metadata and downloaded with the same pinned Rustup 1.28.2 bootstrap.
All advertised archive variants and metadata identities were checked. No original
prototype file was imported or edited.

The combined generation contains **34 recorded files plus its generated inventory**,
**738,543,960 recorded bytes**, and two retained toolchain records. Every file from
the previous 1.90.0 snapshot remains byte-identical. The [generation measurements](multi-version-generation.json)
record its inventory SHA-256:
`5cf6cda5c4c82c053839b10f91f39a21f38c2a1059927cdb873d7c96e23d1cdc`.
This is real downloaded-data composition, not fabricated version metadata.

## Offline coexistence and controls

The [native summary](multi-version-summary.json) records **22 client/control commands**:

- A network namespace with only loopback, no route and external connection refusal.
- Publication through the local HTTP PUT/GET fixture, preserving the static prefix.
  This is **not an actual Nexus schema 3 qualification**.
- Fresh Rustup 1.28.2 bootstrap/home, self-update disabled, Rust 1.89.0 installed,
  compiler/Cargo versions checked, dependency-free offline build/run passed.
- Rust 1.90.0 selected from the same mirror and installed into the same home;
  its version and offline build/run passed. Selecting/building 1.89.0 again passed.
- Missing and corrupted 1.89.0 archive controls failed correctly in separate empty homes.

The output-containment guard was added during review after the first assembly.
Final-source Rust 1.90 reassembly ran inside `unshare --net` and produced an
**identical inventory and verified payload identities**. The unchanged client
bytes support carrying forward the coexistence proof; no duplicate downloads or
installation matrix were run. This composite sequence is preserved in the packet.
Actual native execution is only the recorded GNU x86_64 host/toolchains. It does
not qualify Windows/macOS/musl/ARM installers, old hosts or arbitrary Cargo projects.

## Identities and retained proof

[Source inventory](multi-version-source.json) SHA-256:
`87e37d5c83055b17e78307d8375179a0c545ed56305485e4a5fa80bb989bf649`.
The final Git ref and hosted checks are recorded in the PR.

Sanitized packet `rust-mirror-collection-proof-20261110/multi-version-proof.tar.gz`:
78,617 bytes, SHA-256
`d5ee5b85678c108bf90b5fbc725d36c1c88c289e29692e915e5bafab2d071759`.
It includes source, command records and bounded logs, excluding downloaded
archives, installed homes, credentials/private keys and server data.

## Remaining gaps

No automatic update/resolution during assembly, scheduled sync, retention/pruning,
resumable assembly/upload, bootstrap-version migration or client URL switching.
Source generations must be complete and share one Rustup bootstrap version.
Use private trusted local storage; source hashes/copy checks are not signing trust,
hostile-filesystem protection or power-loss/NFS guarantees. Current actual/deployed
Nexus qualification, Community Edition onboarding and independent signatures remain
separate. All shared instructions, original prototype and unrelated repositories
are preserved. No crate publication, source tag or GitHub Release was performed.
