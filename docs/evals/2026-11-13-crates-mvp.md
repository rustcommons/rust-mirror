# Crate mirroring MVP qualification

Phase 1 implementation checkpoint: `f7c7453` on `feat/crates-mvp`. This is
unreleased development source, not a qualification of published 0.3.0 or a release
approval. All named owner gates remain mandatory. See [contract](../CRATES.md).

## Dated corpus and local observations

The frozen [100-root corpus](../../tests/fixtures/crates-top100-20261113.json)
comes from one official `crates.io/api/v1/crates?page=1&per_page=100&sort=downloads`
response, server date 2026-11-13 04:10:47 UTC. Response SHA-256:
`6ca0d5f602106127fbcc5fda30cf12c56b69e6d84f2387a2e78fa878eaff5b18`.
Each root uses that response's `max_stable_version` with defaults enabled. No roots
are excluded. This dated ranking is not a claim about future download order.
There is no repeated ranking crawl in CI.

Local development-tree observations preceding the checkpoint:

| Check | Bounded result |
| --- | --- |
| Full-index and proposed filtered-index Cargo `generate-lockfile` | 100/100 roots succeed; native and Cargo union both contain 186 versions. |
| Frozen MVP plan | `ef9b2a2ed99eb057828ee8efb057e4d5beed50d63cfb57e161fb5b809b95514c`; index metadata only during planning. |
| Verified archive bytes | Native 45,125,358; Cargo lockfile union 45,125,358; extra 0 bytes, 0% payload-byte overfetch. Metadata/cache/disk blocks excluded. |
| Completed inventory | `66208ace86031cef83c0de23f3e1122913b0f6cb922f14fc0accd3f6d639a093`. |
| Linux HTTPS Cargo clients | Isolated homes, Cargo/rustc 1.90, AArch64 Linux: itoa 1.0.15 and the corpus's itoa 1.0.19 fetch/build with existing locks, then offline run. Not all corpus crates compiled. |
| Negative clients | Untrusted CA, withdrawn archive, corrupted archive and fresh offline cache refuse; populated offline run succeeds. TLS verification stays enabled. |

These observations used evolving development binaries, **not an assertion that
`f7c7453` itself passed the complete qualification matrix**. The new MVP CI repeats
frozen planning, real-Cargo conformance, archive verification/measurement and client
proofs from an exact checked-out revision. Final run/head references must be added
before marking Phase 1 complete.

The first full fetch correctly remained private after final validation rejected
SemVer `+build` archive paths through a toolchain-only validator. The crate reader
now permits only exact archive paths derived from validated crate names/SemVer;
existing toolchain path rules are unchanged. Its regression fixture passes. Explicit
resume rehashed the completed private archives and finished with zero new downloads.
The initial failure is historical evidence, not a current success claim.

The broadened Cargo fixture found a dependency cycle where a native existence
search could backtrack to an older version, while Cargo refused after its own
selection. Cycles now fail the native completeness gate instead of claiming that
existence alone predicts Cargo success. The original Phase 0 record remains intact.

## Current source checks and remaining gates

Focused catalog/native/mirror tests pass, including saved-preview consent without
refresh, immutable output/corruption refusal, partial transfer resume, three retained
generations/yank observations, SemVer build paths, destination mismatch and stale
pruning/paused predecessor protection. Existing 31 retention tests also pass.
Clippy over all targets and denied-warning rustdoc passed before the checkpoint.
These checks do not substitute for process termination, platform clients or Nexus.

At that initial checkpoint, still required: exact-head CI including Windows HTTPS Cargo clients (macOS when
practical), killed-process recovery on Linux/Windows, actual disposable Nexus Raw
publication/client consumption, final link/render/source checks, package audit and
full repository checks. No actual Nexus result is claimed here. No production Nexus
change, new dependency, version bump or publication is authorized.

## Initial exact-head CI, 2d620d7

The [MVP run](https://github.com/rustcommons/rust-mirror/actions/runs/43380261336)
at `2d620d7eb6606cb05a89d5a835a60cb62512c43a` passed top-100 frozen-plan
conformance/archive-byte measurement and Linux **and Windows** real Cargo HTTPS
clients, hard process kill/resume, cooperative stdin cancel/resume, nonzero Range
reuse, typed Retry-After notices and blocked-read heartbeats. Payloads in the
kill/resume fixture are synthetic opaque bytes, not Cargo archives.

Windows initially refused the fixture leaf with unknown revocation status. The
fixture now serves a valid signed local CRL through its certificate distribution
point; certificate and revocation checks remain enabled. This follows the
[Windows chain revocation model](https://learn.microsoft.com/en-us/windows-server/identity/ad-cs/configure-certificate-path-validation-revocation-policy).
The first macOS attempt failed fixture certificate generation/verification using
the system executable; qualification now selects the hosted image's preinstalled
OpenSSL 3 when available. That first macOS attempt is not a client pass.

The actual isolated Nexus Raw publication/client step in this run succeeded. Its
artifact upload then failed because the evidence directory was root-owned. The
workflow now returns ownership of only its own completed proof directory to the
runner so sanitized evidence can be retained; a fully green run with those records
was required and is recorded below. It does not change server permissions or production data.

## Qualified runtime source and complete MVP gates

Runtime implementation `cf3ee3b00017a3d59ec672147add577f3212c136` passed the
[MVP qualification matrix](https://github.com/rustcommons/rust-mirror/actions/runs/43382062984),
[Linux/Windows core checks](https://github.com/rustcommons/rust-mirror/actions/runs/43382062979)
and [native/Cargo fixtures](https://github.com/rustcommons/rust-mirror/actions/runs/43382063100).
The [trimmed machine-readable record](crates-mvp-qualification.json) preserves
exact metrics, schema-4 publication summary, proof cases and resource cleanup.
Final documentation/test-only descendants retain this runtime evidence; their
exact review ref and CI are reported in the PR.

- All 100 dated roots pass full/filtered Cargo conformance; native and Cargo union
  contain 186 versions and **45,125,358 archive bytes** each: **0% archive-byte
  overfetch**. Archive hashes/lengths are rechecked independently by the measurement
  script. All 174 shared original sparse metadata files were independently checked
  against the frozen plan observation hashes; CI now enforces that equality before
  comparing snapshots. Plan ID `a0e3a7544fa8389d271312650cdc02d9b3534e39ed3f4923e5191f4eeef34256`;
  inventory `2c7f228cf9bee1bdef7c31543577913064b4ca52f8c861815bbbf297e2ad9561`.
- Linux, Windows and macOS clients pass saved-preview consent, real HTTPS Cargo
  fetch/build with an unchanged lock and populated offline run. Untrusted CA,
  missing/corrupt archives and fresh offline use refuse. Only itoa is built in these
  client cases; the 100-root resolution/byte corpus is a separate scope.
- On all three systems, the actual CLI survives hard process termination and
  cooperative stdin cancellation by preserving private initialized state; explicit
  resume uses a nonzero Range and verifies the final SHA-256. Retry-After has a typed
  notice and heartbeats continue while reads are blocked. Fixture bytes are synthetic.
- Actual disposable Nexus Raw schema-4 publication verifies four files and 15,025
  GET bytes with the inventory marker last. Repeated publication writes nothing,
  server redeployment refuses overwrite, scoped auth/TLS controls pass, and a fresh
  isolated **Linux** Cargo client fetches/builds via that actual backend. Inventory:
  `c42420c463bd0c9847d81ac811bb1731ae368a84dc5490b132bb2f2fb48e69ac`.
  The namespace has only loopback, external connection refuses, and the owned
  container and volume were removed. This is the named legacy test image with
  temporary TLS termination, not production/Community Edition/native-Nexus-TLS proof.
- Local full checks at that runtime source pass 166 Rust tests (none ignored), 26
  Python tests, formatting, Clippy over all targets and denied-warning rustdoc.
  Follow-up tests strengthen pinned-feature/paused-generation protection and reject
  a historical/current index combination requiring an unmirrored feature edge.
  The conservative combined-index gate may refuse such mixes; it does not silently
  drop history or claim unrestricted Cargo resolver equivalence.

## Package and documentation audit

The clean local development package at `cf3ee3b` was verified by Cargo 1.90 on
AArch64 Linux. SHA-256:
`9983d6648c19551f69db47ace8b1b7b7e4e91824c3aef5b265093f57b68bcf3c`.
All 38 source/package files (including Cargo.toml.orig, lock, README and changelog)
match that Git revision byte-for-byte; normalized Cargo.toml and VCS metadata are
Cargo-generated. This is **not** the immutable published 0.3.0 archive and must not
be uploaded as 0.3.0. Later packaged-documentation edits require their own refreshed
clean package audit before handoff. Historical release records remain unchanged.

Maintained commands/options, source claims, requirement links, Markdown links and
rendering are checked together. `ROM-REQ-049`/`050` retain their identifiers. No new
Rust dependency, version bump, package publication, tag, GitHub Release, production
server write/delete or automatic activation is part of this work. Private registry
identities/authenticated Cargo readers, lockfile import, in-generation compaction,
workspace cleanup and remote pruning retain their documented deferred boundaries.


## Final handoff checks and refreshed development package

Runtime/source checkpoint `de2a4324ddbc72ef2d029e29375b59eb2314e52d` adds strict
host/port parsing without network/environment discovery, keeps the preview's yank
flag current across later generations while preserving historical index bytes, and
completes successful upload/HEAD event operations. Full local checks pass **168 Rust
tests, none ignored, and 26 Python tests**, formatting, Clippy over all targets and
denied-warning rustdoc. The crate fixture suite now has 17 tests, including exact
pinned-profile protection, stale disk, incomplete cross-cohort refusal and malformed
output URLs. No dependency/version/lockfile changes accompany these fixes.

The [refreshed package manifest](crates-mvp-package-audit.json) records all **38**
source/package files checked byte-for-byte against `de2a4324`, plus the clean VCS
marker and generated Cargo metadata. Cargo verified this development archive:
`b6023b91eb3bbcc8037a2743ccb56bb567e9ec4e8fc8240e15c03d018400c16b`.
This supersedes the earlier development audit for handoff, without rewriting it.
It remains an unpublished development audit with the unchanged 0.3.0 manifest
version; it is not the registry 0.3.0 archive and must never be uploaded as that
version. Final evidence-only descendants preserve all packaged source bytes.

The final PR CI repeats the named qualification on the review ref, additionally
checking that standalone schema-4 publish preflight performs no PUT and that
shared native/Cargo full-index bytes match the frozen observation hashes. The
exact final ref/run links live in [PR #33](https://github.com/rustcommons/rust-mirror/pull/33).
The source is held for independent review; no merge/publication action is implied.


The final clean packaged source is `03647787a2543b000e4fedd086be4a903e5a513d`;
archive SHA-256 `cce2744ed45864355a8d592adb42a0de971c99865d57b109dd8e2eb891346c3e`.
The manifest preserves the earlier audit records as history and verifies all 38
current source files plus clean VCS metadata. This final source also refuses the
toolchain CLI's prune route for crate leaves, requires the crate catalog policy,
and turns partial CLI crate-prune failures into typed I/O results with their
successful-prefix report. The generic engine remains shared; it does not infer
crate pins for arbitrary library callers. Focused crate and existing retention
checks pass, including both CLI route refusals and untouched crate generations.
The final evidence-only descendant changes no packaged bytes. CI on the exact PR
review head remains the handoff gate, with the qualification runs linked in the PR.
