# CLI glibc compatibility measurements — 2026-11-15

## Source, methods and publication boundary

The owner approved an AlmaLinux8 GNU download alongside Windows static CRT, Linux
static musl and both macOS architectures, starting with the NEXT release. v0.5.1
keeps its four published archives and original SHA256SUMS. No archive/manifest/tag,
version, crate dependency or product source was changed by these measurements.

[Five-archive baseline dry run](https://github.com/rustcommons/rust-mirror/actions/runs/43726536265)
and [CI-only measurement run](https://github.com/rustcommons/rust-mirror/actions/runs/43726536233)
passed at automation source 62a399beda85ab439c440e4709268ac8a32de509. The measurement
job's green status means every outcome was recorded, not that every target/userland
was compatible. [Machine-readable results](glibc-measurements-20261115.json) contain
all 40 combinations, actual image IDs/digests, raw bounded failure reasons, command
arguments, symbol requirements and script/workflow hashes.

Application source was immutable v0.5.1 commit
112ef7a7a71d40349c601f299f965199b5b279ba; Rust/Cargo 1.90.0. Ordinary baseline:
almalinux:8@sha256:2d0f2eb2a83cc09dbc2eb92fade4dbe8f709628149e81aa00aec70630406603b,
GCC8.5.0, glibc2.28 and Python3.9 inside the build container. Its maximum required
GLIBC symbol is 2.28, and bare Alma8 version + real official crate TLS/sync passed.
Host Python3.12 packages the resulting archive without requiring tomllib in Alma8.

Experimental tool: [cargo-libc e68ca7b](https://github.com/rustcommons/cargo-libc/tree/e68ca7bf3a8ffd973c8576acc48abd0fbcc66d25),
version0.7.0, Clang18 and host GCC13 on Ubuntu24.04. Commands use its target spelling:

```sh
cargo libc build --target x86_64-unknown-linux-gnu.2.17 \
  --cc /usr/bin/clang-18 --artifact "$ARTIFACT" -- --release --locked
```

Each target uses a separate CARGO_TARGET_DIR; the ordinary Alma8 binary is also
run through the identical userland corpus. Existing C dependencies compile through
the first-party tool's wrappers. Both experimental builds passed their ABI checks:
actual maximum GLIBC requirements are 2.28 and 2.17 respectively. Both need ordinary
libc/libm/libpthread/libgcc_s/libdl runtime libraries; no OpenSSL runtime is used.

## Every measured combination

Every **Pass** below includes --version, a fresh real itoa1.0.15 preview, whole-plan
approval and sync through index.crates.io/static.crates.io with rustls verification
and bundled public roots. A loader failure stops before planning; it is not recorded
as a network failure. Target2.16 is rejected before building, so its ten runtime
combinations are explicitly not run. All ten public images were obtainable and
started successfully for their own glibc probe.

| Runtime userland (actual glibc probe) | Ordinary Alma8 build | cargo-libc 2.28 | cargo-libc 2.17 | cargo-libc 2.16 |
| --- | --- | --- | --- | --- |
| AlmaLinux 8 (2.28) — glibc 2.28 | Pass: version + real HTTPS plan/sync | Pass: version + real HTTPS plan/sync | Pass: version + real HTTPS plan/sync | Not run: target below cargo-libc 2.17 limit |
| centos:7 — glibc 2.17 | Loader failure: missing GLIBC_2.18, GLIBC_2.25, GLIBC_2.28 | Loader failure: missing GLIBC_2.18, GLIBC_2.25, GLIBC_2.28 | Pass: version + real HTTPS plan/sync | Not run: target below cargo-libc 2.17 limit |
| opensuse/leap:42.3 — glibc 2.22 | Loader failure: missing GLIBC_2.25, GLIBC_2.28 | Loader failure: missing GLIBC_2.25, GLIBC_2.28 | Pass: version + real HTTPS plan/sync | Not run: target below cargo-libc 2.17 limit |
| ubuntu:20.04 — glibc 2.31 | Pass: version + real HTTPS plan/sync | Pass: version + real HTTPS plan/sync | Pass: version + real HTTPS plan/sync | Not run: target below cargo-libc 2.17 limit |
| ubuntu:18.04 — glibc 2.27 | Loader failure: missing GLIBC_2.28 | Loader failure: missing GLIBC_2.28 | Pass: version + real HTTPS plan/sync | Not run: target below cargo-libc 2.17 limit |
| ubuntu:16.04 — glibc 2.23 | Loader failure: missing GLIBC_2.25, GLIBC_2.28 | Loader failure: missing GLIBC_2.25, GLIBC_2.28 | Pass: version + real HTTPS plan/sync | Not run: target below cargo-libc 2.17 limit |
| debian:9 — glibc 2.24 | Loader failure: missing GLIBC_2.25, GLIBC_2.28 | Loader failure: missing GLIBC_2.25, GLIBC_2.28 | Pass: version + real HTTPS plan/sync | Not run: target below cargo-libc 2.17 limit |
| debian:8 — glibc 2.19 | Loader failure: missing GLIBC_2.25, GLIBC_2.28 | Loader failure: missing GLIBC_2.25, GLIBC_2.28 | Pass: version + real HTTPS plan/sync | Not run: target below cargo-libc 2.17 limit |
| debian:7 — glibc 2.13 | Loader failure: missing GLIBC_2.14, GLIBC_2.16, GLIBC_2.17, GLIBC_2.18, GLIBC_2.25, GLIBC_2.28 | Loader failure: missing GLIBC_2.14, GLIBC_2.16, GLIBC_2.17, GLIBC_2.18, GLIBC_2.25, GLIBC_2.28 | Loader failure: missing GLIBC_2.14, GLIBC_2.16, GLIBC_2.17 | Not run: target below cargo-libc 2.17 limit |
| centos:6 — glibc 2.12 | Loader failure: missing GLIBC_2.14, GLIBC_2.16, GLIBC_2.17, GLIBC_2.18, GLIBC_2.25, GLIBC_2.28 | Loader failure: missing GLIBC_2.14, GLIBC_2.16, GLIBC_2.17, GLIBC_2.18, GLIBC_2.25, GLIBC_2.28 | Loader failure: missing GLIBC_2.14, GLIBC_2.16, GLIBC_2.17 | Not run: target below cargo-libc 2.17 limit |

## Results and proposed next step

Ordinary Alma8 and cargo-libc2.28 each passed 2/10 userlands: Alma8 and Ubuntu20.04.
cargo-libc2.17 passed 8/10, additionally CentOS7, openSUSE Leap42.3, Ubuntu18.04,
Ubuntu16.04, Debian9 and Debian8. Debian7/glibc2.13 and CentOS6/glibc2.12 refused
with explicit newer-version loader requirements. The oldest successful glibc
actually measured was 2.17, not a claim that no lower floor is ever possible.

The next requested lower target, 2.16, exits1 with:

```text
cargo-libc: supported glibc target releases start at 2.17 (glibc 2.x only)
```

This matches the [tool's documented range](https://github.com/rustcommons/cargo-libc/blob/e68ca7bf3a8ffd973c8576acc48abd0fbcc66d25/README.md).
No cargo-libc bug was observed in the measured builds/operations. The exact lower-
bound command/output, source/tool revision, CI logs and successful application
matrix were reported to its maintainer. Lower targets require tool support first.

**Proposal to the coordinator:** use the proven cargo-libc2.17 method for the next
GNU download: it reaches six additional old userlands beyond the Alma8 baseline.
No promotion has been made; the publication workflow remains on ordinary pinned
Alma8 until separately approved. Every future tagged production binary still needs
its own qualification; these are measurements of this exact source/tool/corpus.

## Limits and reproducibility

These are old **userlands on a shared modern CI kernel**, not real CentOS/SLES/Debian
old kernels or a seccomp kernel simulation. Exact host kernel/image identities are
in JSON. No universal old-kernel/CPU/NSS/SSO or arbitrary project behavior is proven.
DNS/TLS passed for the named official hosts under each passing image's resolver;
this is not packet inspection or all enterprise certificate/proxy policies.

openSUSE Leap42.3 is an openly available community
[SLE12 SP3-aligned userland](https://news.opensuse.org/2017/07/26/refresh-of-linux-distribution-continues-leveraging-community-enterprise-benefits/),
with measured glibc2.22; it is a proxy, not native commercial SLES qualification.
No commercial subscription or EULA was accepted/bypassed. Old images are fixtures,
not recommendations for production deployment. Runtime containers were read-only,
non-root, uniquely named and removed; only owned writable state held mirror runs.

The experiment workflow bootstraps at PR opening and is manually dispatched after
material measurement changes. Results-only commits carry identical script/workflow
SHA-256s rather than repeat this unchanged expensive matrix. Core/packaging CI and
the five-archive dry run remain independent gates. No experimental artifact is
automatically attached to a release. Later approval/resolution must be appended
without rewriting this initial result or v0.5.1's existing publication evidence.
