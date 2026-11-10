# Resumable-download iteration: 2026-11-10 UTC

The owner authorized resumable downloads after PR7/8 merged. This iteration adds
explicit same-plan `fetch --resume`, reusable verified payloads, partial byte-range
continuation and output locking. It does not prepare a crate release, alter the
legacy fetch route or implement resumable uploads, updates or retention.

## Assumptions and design checks

The upstream origin must permit useful ranges, but HTTP permits servers to ignore
Range. [RFC 9110 range semantics](https://www.rfc-editor.org/rfc/rfc9110.html#name-range-requests)
therefore inform explicit restart handling for `200` and `416`. Coherent `206`
responses are checked before appending; malformed/oversized/truncated responses
cannot certify completion. Whole-file SHA-256 remains the integrity authority;
we do not claim ETag/If-Range identity or universal server behavior.

The pinned [Panamax downloader](https://github.com/panamax-rs/panamax/blob/84ca2d7064f5a9931371f9eb74c4003707897a7b/src/download.rs)
was inspected as a reference for existing-file hash checks and temporary partial
files. No code was copied and no general parity/security claim is made.
Rust's [File locking API](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)
is used without another dependency; Rust 1.90 compilation and native tests passed.

The selected API behavior is documented in [RESUMING.md](../RESUMING.md). Existing
output refusal stays the default. The explicit flag authorizes only a validated
workspace for the same canonical plan identity; it is not an overwrite switch.
Only completed snapshots are publishable. Small workspace identity/lock files
remain after success; payload work files are removed.

## Actual upstream observations

[Twelve bounded probes](resumable-download-ranges.json) covered the recorded Rust
1.90.0 GNU/Wasm toolchain's gzip/xz archives for Cargo, Clippy, rust-src, rust-std
and rustc. Every official HTTPS request for bytes 1024?2047 returned `206`, the
requested range, a known total and exactly 1,024 bytes. Only those small slices
were read, not twelve full archives. These observations support the recorded
origin/date, not every server, proxy or artifact.

## Failure and process tests

Permanent tests cover sixteen local HTTP responses across valid/full transfers,
ignored ranges, `416`, malformed/missing/overflowing ranges, inconsistent lengths,
wrong encodings, redirect refusal, truncated and oversized streams.

The workspace tests exercise partial append, full restart, rehash/reuse of completed
files, detection/recovery of corrupted partial and complete cache entries, a fully
received partial before promotion, changed-plan refusal, unknown state/paths,
competing locks, Unix symbolic links/backslash paths and completed-output integrity.
An actual child test process is killed during a transfer on Linux and Windows;
resuming succeeds after the OS releases its lock. No other process is killed.

During development the ignored-range Windows test caught that an append-only file
handle could not truncate. The final implementation opens read/write and explicitly
seeks before append/restart. The regression now passes on both systems. Existing
corruption tests were adjusted to damage all identical-content variants, since a
verified content-addressed file can legitimately satisfy another identical path.

- Windows Rust 1.99: all 40 Rust tests passed; fmt, Clippy and warnings-denied docs
  passed. Python ran 11 tests with one existing symlink-privilege skip.
- Linux Rust 1.90: all 42 Rust tests passed; Clippy, warnings-denied docs and all
  11 Python tests passed. Unit/CLI stdout was captured separately in a narrow
  follow-up because the first combined WSL stream interleaved some summary lines.
- Source/manifest/lock identities are in [the source inventory](resumable-download-source.json),
  SHA-256 `a1ff79b3dc23cac7a0a9bb55205b32b5994ce7f467ea4667869d9f0d088d6455`.
  No dependency, version, MSRV or license changes.

## Real official-origin interruption and resume

[The permanent driver](../../scripts/prove_resume.py) used the previously frozen
Rust 1.90.0/Rustup 1.28.2 GNU selection with Clippy, rustfmt, rust-src and Wasm.
It started the new CLI against a fresh output and killed only its own child while
Cargo's 15,110,842-byte gzip archive was incomplete. The retained prefix was
**274,736 bytes**; the interrupted process exited `-9` and no `mirror` existed.

The remaining fourteen payloads were copied from the earlier verified baseline
and checked against the frozen hashes. This intentionally avoided another full
420 MB network transfer. The resumed CLI logged the interrupted offset, fetched
its tail and reused the others. [The result](resumable-download-summary.json)
passed: **all 23 output files including inventory exactly match the prior baseline**.
Inventory SHA-256 is unchanged:
`3f0a02543c2eadc69b2e8c973ecd7508bb48706f05cbe1f70f8198821bd80bdf`.

A subsequent `fetch --resume` on that completed output succeeded inside a fresh
root-owned `unshare --net` namespace, with no external network, proving that the
completed-tree path does not need new downloads. No Rustup install was repeated:
matching bytes preserve only the earlier installation proof's exact scope.
The driver logs a requested resume offset; it does not measure wire-byte totals.

The retained sanitized packet is `rust-mirror-resume-proof-20261110/resumable-download-proof.tar.gz`,
59,633 bytes, SHA-256
`043e24ce9ec2fb7150c2befa9b0a3fef2715081bc9f733798794b24123de01c4`.
It contains source, inventories and bounded logs, not downloaded toolchains,
credentials, private keys or user homes. Hosted CI and final commit are recorded
in the PR rather than inferred from local checks.

## Limits and next work

Resume is explicit and per selected output, not automatic retry or shared-cache
synchronization. It uses private trusted local storage; NFS/multi-machine locking,
hostile mutation, power-loss durability and interrupted initial identity setup are
not qualified. There can be a temporary payload-space doubling during assembly.
Existing file/byte limits apply to reused bytes too. A damaged completed snapshot
is refused without repair or overwrite.

Legacy downloads, resumable HTTP/Nexus publication, retention/update policy,
independent OpenPGP checks and foreign-host runtime qualification remain separate.
No production service, server EULA, original prototype, user toolchain home or
release was changed by this iteration.
