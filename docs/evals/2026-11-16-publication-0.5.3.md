# rust-mirror 0.5.3 publication checkpoint — 2026-11-16

## Verified registry publication

The coordinator normally merged [PR #60](https://github.com/rustcommons/rust-mirror/pull/60)
at `e74f91b3f39f7ff78675f184b75c5602d681d377`, tree-identical to qualified
`406ca37de815ccdb871a39572269a95d5e6cea6e`. After the owner's crates-io environment
approval, [Trusted Publishing 43782529509](https://github.com/rustcommons/rust-mirror/actions/runs/43782529509)
succeeded. Official crates.io trustpub_data independently identifies provider github,
repository rustcommons/rust-mirror, that run and that merge. Registry creation time
is 2026-11-16T01:50:20.730945Z, distinct from the 2026-11-15 authorization. No
long-lived Cargo token was used.

Published archive SHA-256:
`1add596eec357351f05138fa6ea534630346af28340cc3b06c136f19266f82be`.
API, sparse index, downloaded archive and clean merge package agree; non-yanked.
All 45 members are regular files and all 43 original Git payloads match the merge.
Normalized Cargo.toml matches the qualified candidate; only .cargo_vcs_info.json
differs between candidate and published payload, identifying the clean merge.
Candidate checksum
`775a1b5788b378fd35c0deeabff7fdcf61e1219e428b319b630c4bf5832d4306`
remains historical, not the published checksum.

Annotated v0.5.3 object `7ade8221533935bf92953bcf2476d2ba58967606` peels to the
exact merge locally and remotely. The public, latest, non-prerelease
[GitHub Release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.5.3)
was created from the exact tagged notes. It has **no binary assets** at this
checkpoint. Existing v0.5.1/v0.5.2 IDs, note bodies, asset IDs/sizes/digests still
match the saved prepublication snapshot; no manifest, asset or protected tag changed.
The [JSON record](publication-0.5.3.json) retains exact source, run and verification data.

## Documentation and installed proofs

The exact docs.rs [API](https://docs.rs/rust-mirror/0.5.3/rust_mirror/),
[events](https://docs.rs/rust-mirror/0.5.3/rust_mirror/events/),
[crates](https://docs.rs/rust-mirror/0.5.3/rust_mirror/crates/) and
[lock-import](https://docs.rs/rust-mirror/0.5.3/rust_mirror/crates/lockfile/)
pages returned HTTP 200 directly and contain 0.5.3. Local published-release checks
passed for all nine versions, as did
[documentation run 43800066952](https://github.com/rustcommons/rust-mirror/actions/runs/43800066952).

A fresh actual cargo install rust-mirror --version 0.5.3 --locked passed on
Linux/aarch64 with Cargo1.90, a new Cargo home/target/install root, official archive
checksum, version/protocol1 and real itoa1.0.15 preview/approved fetch. Independently,
[registry run 43799990830](https://github.com/rustcommons/rust-mirror/actions/runs/43799990830)
passed on Linux/x86-64: fresh install, checksum/clean merge VCS, protocol discovery,
installed publisher in a loopback-only namespace, TLS/auth/remote-exists refusals
and fixture-key destruction. This fixture is not actual Nexus or a fresh full
Cargo client/conformance qualification.

## Binary attempts and precise blocker

The original [tag run 43782529527](https://github.com/rustcommons/rust-mirror/actions/runs/43782529527)
passed native builds but the collector's bounded GitHub Release wait expired with
HTTP404 at 2026-11-16T01:05:33Z, before owner registry approval completed. Attestation
and upload were skipped. Preserve this initial observation.

After creating the exact-note empty release, the coordinator authorized a fresh
non-dry dispatch at immutable v0.5.3:
[43799962003](https://github.com/rustcommons/rust-mirror/actions/runs/43799962003).
All five native builds, unpacked smokes and eight GNU launch/real HTTPS plan-sync
userland gates passed. The collector created five archives, one CycloneDX SBOM and
one SHA256SUMS with six entries. Provenance signing succeeded, but the SBOM signing
step failed: **Unsupported SBOM format. Must be valid SPDX or CycloneDX JSON.**
Upload was skipped; these are workflow artifacts, not public release downloads.
Seven-file live provenance/SBOM verification and actual public Windows/GNU/musl
execution have therefore **not** completed, and no SLSA3 badge is claimed.

The actual generated CycloneDX1.5 document has 127 dependency components and
omits serialNumber. The pinned
[action detector](https://github.com/actions/attest/blob/59d89421af93a897026c735860bf21b6eb4f7b26/src/sbom.ts)
requires bomFormat, serialNumber and specVersion. The original validation checked
format/root/graph/source but missed this action-specific requirement.

## Generator repair and measured scope

The repair adds a canonical UUID URN serialNumber, deterministically derived with
UUIDv5 from the complete canonical document excluding any old random serial.
Changes to source, lock or document content change its identity; temporary source
paths and prior random serials do not. This is document identity, not a security
signature. Validation now rejects missing/malformed serials before collection or
signing, and generation validates the final canonical document.

Regenerated twice from the clean e74f91b release source using the actual pinned
cargo-cyclonedx0.5.9 binary. Both documents are byte-identical; compared with the
failed workflow's actual SBOM, **only serialNumber differs**. New SBOM SHA-256:
`999c8a0cdd2159cedd451c3d8c08ad3c5533ae93b1208a4ae60e62bc261763c2`;
serial `urn:uuid:1ea8b964-ed70-5443-995d-5be6848894e1`. The 127 component identities,
checksums, source/lock properties and dependency graph are unchanged. This proves
the pinned detector's required fields are present, not successful live signing.
Regression tests cover absent/malformed serials, random-serial/path independence,
content/source changes and the existing checksum/graph/collection refusals.
All 57 Python tests pass; binary-based protocol tests use the freshly installed,
verified 0.5.3 registry executable because application source is unchanged.

## Recovery requiring review

The fix on main cannot change the protected tagged workflow. Re-running v0.5.3
unchanged will reproduce the failure. A dispatch from a repaired main revision
would have a different GitHub workflow/OIDC source and signer digest; current
verification deliberately requires both equal the release merge. Do not silently
relax those checks, move the tag, substitute unreviewed workflow artifacts or upload
manually to bypass them. The coordinator must review the repair and approve an
explicit recovery policy separating immutable application source from a reviewed
builder revision before another live signing/upload attempt. Then verify all
seven files, all five SBOM bindings and real public Windows/GNU/musl downloads,
and append the resolution with exact revisions/runs to this checkpoint.

The repository advisory GHSA-46c8-w6pp-97pw remains **draft**, unpublished, for the
coordinator. Release notes retain all upstream advisory references and bounded
exposure; registry success alone does not complete this release's binary gates.
Candidate paired reproducibility belongs to 406ca37, not this production dispatch.
GNU tests prove container userlands, not old kernels/commercial SLES; macOS native
smokes are not public-download/Gatekeeper UI evidence. OS signing/notarization
remain absent. This repair changes release tooling/tests and excluded records,
not application Rust source, versions, dependencies or any published artifact.

## Subsequent owner decision — 2026-11-16

The owner rejected a recovery exception for 0.5.3 and authorized a new 0.5.4
release with the merged serial fix. The earlier recovery proposal above is a
historical unapproved proposal; do not execute it. Version 0.5.3 remains crate-only
with no prebuilt assets. Preserve its protected source, exact GitHub Release body
and both failed attempts. See [0.5.4 notes](../releases/v0.5.4.md) for the intended
first patched-download profile; no 0.5.4 publication is claimed at this checkpoint.
