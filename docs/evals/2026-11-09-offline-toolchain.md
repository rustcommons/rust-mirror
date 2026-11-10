# Pinned offline toolchain proof - 2026-11-09

## Result

**Passed within the scope below.** A fresh Rustup bootstrap installed the pinned
minimal Linux toolchain entirely through a local HTTP server under a Nexus-style
Raw repository prefix. Cargo then built and ran a dependency-free application
without external network access. No real Nexus server was contacted.

- Rust: `1.90.0`, `rustc 1.90.0 (1159e78c4 2025-09-14)`.
- Cargo: `1.90.0 (840b83a10 2025-07-30)`.
- Rustup bootstrap: `1.28.2 (e4f3ad6f8 2025-04-28)`.
- Host: `x86_64-unknown-linux-gnu`, native execution in the existing Ubuntu WSL userland.
- Profile: `minimal`; selected archives: Cargo, rustc and host rust-std.
- Upstream payload: eight files, 144,153,726 bytes, plus the generated inventory.
- [Exact inventory](2026-11-09-offline-snapshot.toml).
- [Final proof summary](2026-11-09-offline-summary.json).
- [Real metadata cases](2026-11-09-manifest-shapes.json): 24 host/package shape
  observations from the official manifest; these are not other-platform install tests.

## What was measured

The connected fetcher read official HTTPS metadata, checked the manifest sidecar,
preserved the full manifest and signature bytes, selected preferred host archives,
verified streamed archive checksums and downloaded the pinned bootstrap. Every file
is recorded in the inventory. SHA-256 verification is mandatory, not a cosmetic
configuration flag. Signature bytes are preserved but OpenPGP is not independently
verified. HTTPS plus upstream checksums is the current source trust boundary.

The final library was rebuilt and replayed against those same recorded upstream
bytes, producing byte-identical inventory/payload identities. A transport adapter
read the recorded files; it did not claim a second internet fetch. This avoids
redownloading 144 MB merely to exercise the final Windows handle-closing correction.
The final offline installation used that replayed completed tree and fresh homes.

The proof driver was run under `unshare --net`. It enabled only loopback, asserted
that `ip -json route` was empty and the only interface was `lo`, and confirmed an
external numeric IP connection failed. It hosted and published content wholly
inside that namespace. It supplied dedicated, initially empty `HOME`, `CARGO_HOME`
and `RUSTUP_HOME`; no host Rust toolchain/cache was added to the client's PATH.
The installed toolchain still used the existing system linker/loader/native libraries.

| Case | Actual result |
| --- | --- |
| Publisher local preflight | Every inventory path, file size and SHA-256 checked; unexpected files/symlinks refused by focused tests. |
| Raw protocol fixture | Nine HEAD absence checks, nine PUT uploads, nine GET byte verifications, nested `/repository/rust-offline/proof` prefix. |
| Bootstrap from served tree | Downloaded from the local Raw-style URL, hash checked, executed without modifying shell startup/PATH. |
| Fresh minimal installation | Exit 0; all three components downloaded from the configured local repository path. |
| Rust/Cargo identity | Exact pinned version commands succeeded. |
| Offline Cargo application | `cargo build --offline` exited 0; the resulting executable printed `offline mirror proof ok`. No dependencies were declared. |
| Missing-archive control | Another fresh client failed installation with HTTP 404 when the Cargo archive was withheld. |
| Corrupt-archive control | Another fresh client failed on a SHA-256 mismatch after a byte was flipped in the served Cargo archive. |
| Isolation and request scope | No route outside loopback; recorded HTTP requests stayed under the configured repository prefix. |

The HTTP fixture is deliberately small. It models static HTTP PUT/HEAD/GET and
no-overwrite behavior; it does not model Nexus authorization, storage, plugins,
cleanup, TLS or deployment settings. The publisher uses a fresh private prefix,
archive-before-manifest ordering, final inventory marker and GET verification.
This is not a multi-file transaction or protection against competing publishers.

## Checks and corrections

- Windows Rust 1.99: 15 Rust tests passed, Clippy with warnings denied, rustdoc with
  warnings denied and formatting passed. Python preflight suite: six pass and one
  explicit skip because this account cannot create symlinks.
- Linux Rust 1.90: 15 Rust tests passed; all seven Python preflight tests passed,
  including symlink rejection. No new third-party package names or dependency
  versions/checksums were introduced beyond the assessed prototype's locked graph.
  `tempfile`, already in that graph, is now used by the preparation library.
- Real install proof: eight positive commands exited 0, and the two deliberately
  failing install controls exited 1. The four negative-client setup commands passed.
  Full command exits and scope are retained in the summary.

An initial live planner attempt correctly stopped before creating output because
its first draft included foreign-target `rust-std` extensions. Selection was fixed
to match the requested host (or wildcard), and a regression test now covers that
case. That failed attempt is retained separately and is not counted as a pass.

The first Windows test run found three successful-publication cases failing with
access denied: the snapshot inventory handle remained open during directory rename.
The final implementation explicitly closes it. All final Windows tests pass. The
Linux metadata/artifact bytes remained the same, as confirmed by replay above.

## Reproduce

Build the CLI with Rust 1.90+, then prepare a new snapshot on a connected machine:

```sh
cargo build --locked
mkdir -p /tmp/rust-mirror-check
target/debug/rust-mirror fetch-toolchain \
  --release 1.90.0 --host x86_64-unknown-linux-gnu \
  --rustup-version 1.28.2 --output /tmp/rust-mirror-check/snapshot
sudo unshare --net python3 scripts/prove_offline.py \
  --snapshot /tmp/rust-mirror-check/snapshot/mirror \
  --evidence /tmp/rust-mirror-check/evidence
```

Use fresh paths. Root/namespace permissions and Python 3.11+ are required for this
proof; the mirroring CLI itself does not require root. The driver shuts down its
server and removes only its own temporary server tree. It leaves evidence and
isolated client installations for inspection. It does not touch the user's homes.
The manual `Offline toolchain proof` workflow carries the same recorded command
shape for future runs; local execution is the evidence claimed here.

## Limits and next integration gate

This is an unreleased milestone, not a general production/release qualification.
Actual Nexus Repository version/configuration, read/write access policy, trusted
CA/TLS and redeployment/cleanup behavior remain untested. Rustup's authenticated
archive downloads are not covered by the publisher's Basic-auth support. Use the
[Nexus guide](../NEXUS-RAW.md) to prepare an instance-specific test.

Only the recorded host/release/client/minimal profile is demonstrated. No rolling
channels, extra components/targets, self-updates, upgrade/migration of existing
homes, Windows/macOS client installation, alternate compression-client selection,
crate/index mirroring, dependency/native-tool closure or GUI has been qualified.
The full upstream manifest advertises more artifacts than this subset contains;
unsupported selections fail rather than being silently fetched from upstream.

No crates.io publication, source tag or GitHub software Release was created.

## Evidence retention

The final log-only proof packet has 32 members, 4558 bytes and
SHA-256 `8a75f87e9c70dc6246d4449cd15c755ade8cce0b05a228f5a5d03a2cc51170e1`. It is retained in coordinator thread storage; it contains logs, summary, original inventory and replay identity, not downloaded executables or installed toolchains.
