# 0.4.0 release candidate — 2026-11-13

## Scope and source

The owner authorized 0.4.0 on 2026-11-13. This is prepublication evidence, not a
registry upload or completed release. The coordinator reviews/merges and publishes
from the owner's machine, then postpublication checks follow [policy](../RELEASING.md).

Candidate source `4169018b9ab700cb8de3ecd94c30db5118d2320a` starts from accepted
main `0a6102b3ff05f459f4597e290012913e2b1c3d92` (reviewed Phase 1 head
`3de809d79e48ea164a6f01882b1c0282b09bdac3`, identical runtime tree). Main CI
[43390263428](https://github.com/rustcommons/rust-mirror/actions/runs/43390263428)
was green before release work. Changes are the package/root lock version,
description, concise README/docs.rs and release/trace documentation, CLI help wording
and the research User-Agent deriving its version from package metadata. Dependency
identities and mirror/solver/transport/publication/retention behavior are unchanged.
A later commit adds this excluded evidence and Linux installed-package CI coverage;
those files are not included in the crate.

## Checks and package

Rust/Cargo 1.90: fmt, 167 Rust tests plus one rustdoc example, Clippy all targets with
warnings denied, rustdoc with warnings denied and 26 Python tests passed. Changed
Markdown links/anchors and GitHub rendering passed. Twenty-seven existing external
URLs returned 200; the future v0.4.0 source tag and main release-notes URL are checked
structurally until merge/publication. Exact common AGENTS sections and CLAUDE bytes
match main. The local release-documentation checker passed all five version records.

[Machine-readable audit](release-candidate-0.4.0.json) records the clean source,
Cargo-generated VCS metadata, all 38 packaged source-file hashes and installed
executable identity. `cargo package --locked` verified the crate: 40 members
including generated Cargo.toml and VCS metadata, no private keys, proof archives or
excluded documentation. Every source member matches the candidate commit; the
original manifest, lock, license, README and changelog are included. Linux/Cargo
1.90 archive SHA-256:
`fe309c4835c93951c6fb366eb80463b45906b915729db3a81c8f180a87c67ac4`.
This is a candidate archive, not a promised checksum for the coordinator's later
Windows/Cargo package. Compare immutable source payloads and record generated
manifest/compression/VCS differences honestly after the normal merge.

Unpacked and installed with `cargo install --path … --locked --root FRESH_ROOT`
using a new Cargo home on Linux/aarch64. The installed binary reported 0.4.0 /
protocol 1, then completed real itoa 1.0.15 HTTPS fetch/build/offline run in private
Cargo homes. Untrusted CA, missing/corrupt archives and a fresh offline home were
refused; the lockfile stayed unchanged. Installed CLI kill and stdin cancellation
preserved 65,536 bytes each and resumed a synthetic 1 MiB payload by range, with
retry notices and heartbeats during blocked reads. This is not an archive-format
proof. Temporary TLS keys were scoped to the fixture and removed on exit.

The local loopback-namespace publisher attempt was blocked by `unshare: Operation
not permitted`; no publication ran there and isolation was not weakened. Linux CI
now installs the verified package into a fresh Cargo home/root and runs the existing
publisher preflight/lifecycle fixture inside its loopback-only namespace, saving
identity/proof artifacts. Actual Nexus evidence remains separately identified below. The first CI attempt
at head `604309509a7687baa5b713fa92f0817ee44fc462` passed the installed publisher
checks but failed while collecting artifacts: the existing Linux fixture leaves
root-owned temporary keys. The follow-up adds an EXIT trap deleting exactly those
two fixture keys, asserts their removal, and uploads only JSON/stdout/stderr proof
files. It does not broaden permissions or include keys in artifacts. The failed
run [43418858382](https://github.com/rustcommons/rust-mirror/actions/runs/43418858382)
remains historical; require the corrected final-head CI before release.

## Carried evidence and limits

[Phase 1 qualification](2026-11-13-crates-mvp.md) remains applicable to unchanged
runtime code: 100/100 dated roots, 186 native/Cargo-union versions and 45,125,358
archive bytes on each side (0% overfetch), zero planning archives, Linux/Windows/
macOS HTTPS and resume cases, and actual disposable schema-4 Nexus publication with
Linux Cargo consumption. The original exact runs and observations are preserved.
The release PR also automatically runs the existing qualification workflows; its
final checks must be green before handoff.

Client builds cover itoa, not every corpus root or arbitrary targets. Actual Nexus
client proof is Linux-only, on the recorded legacy OSS test instance with a verified
TLS reverse proxy. Historical/current cohorts requiring unmirrored edges are refused,
not silently expanded or exposed incomplete. See [release notes](../releases/v0.4.0.md).
No production server writes, GUI Phase 2, registry publication, tag or GitHub Release
is established by this candidate record.
