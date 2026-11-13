# Crate mirroring MVP qualification in progress

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

Still required: exact-head CI including Windows HTTPS Cargo clients (macOS when
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
is still required. It does not change server permissions or production data.
