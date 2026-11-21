# rust-mirror 0.6.0 publication — 2026-11-21

## Source and Trusted Publishing

The coordinator merged [PR #73](https://github.com/rustcommons/rust-mirror/pull/73) at
`eaa63546de95f617794d2d2d8fa100013dcea523`. Its tree is identical to the qualified
PR head `03ce7ec9fab6463c1f6a5ff19da2e7dd327696ee`, whose candidate dry run
[44905598952](https://github.com/rustcommons/rust-mirror/actions/runs/44905598952)
passed. Annotated tag v0.6.0 (object `c3d28289b64614d9201003ac22713f2fbe92dfa0`)
peels to that merge. After owner approval,
[Trusted Publishing 44906731148](https://github.com/rustcommons/rust-mirror/actions/runs/44906731148)
published the crate. crates.io trustpub_data names github, rustcommons/rust-mirror,
that run and the merge; creation time is 2026-11-21T14:16:00.558826Z.

Registry checksum `beaffa1b9467cd04bd03efe7c3f68fbc3b7d4d042c19175f0115a85ba3eb3275`, not yanked, rust-version 1.90.
`verify_registry_release.py --tag v0.6.0` passed. The published crate has 45 regular
members; the 43 original files are byte-identical to the merge, the normalized
manifest equals the qualified candidate's, and `.cargo_vcs_info.json` names the merge.
The candidate checksum `d1525de52b17a373188f55257890c3e049f341c4c291014b8743fc5577f0dc66` differs only in that VCS
record; do not substitute it for the published checksum.

## Collector timeout and exact-tag recovery

The tag-triggered [release binaries run 44906731143](https://github.com/rustcommons/rust-mirror/actions/runs/44906731143)
built all five targets, then its `verify` job waited the bounded 600 s for the
GitHub Release. It failed at 2026-11-21T13:18:47Z with `existing GitHub Release
unavailable (HTTP 404)`. The cause is ordering, not a defect: the GitHub Release is
created only after confirmed crates.io publication, and publication waited for owner
approval until about 14:16Z. The `attest` and `publish` jobs were skipped, so that
run created no attestations and uploaded no assets. Its history is kept as is.

After confirming publication, the exact committed notes were published as the
[GitHub Release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.6.0) at
2026-11-21T14:18:16Z: public, latest, not prerelease, body equal to
`docs/releases/v0.6.0.md` at the tag, with no assets. The published-tag documentation
check passed. The documented recovery then ran: workflow_dispatch of
release-binaries.yml on ref v0.6.0 with tag=v0.6.0, dry_run=false, candidate=false.
[Run 44913249402](https://github.com/rustcommons/rust-mirror/actions/runs/44913249402)
passed every job: five native builds and smokes, all eight GNU userland gates, SBOM
generation, reusable-workflow attestation, staging and store match of the provenance
bundle, and append-only upload. The workflow commit equals the source commit; no
source/signer exception, new builder or tag move was used.

## Eight live files

This is the first release with the attached provenance bundle. SHA256SUMS has six
sorted entries (five archives and the SBOM). Each GitHub asset digest matches the
downloaded bytes.

| Asset | SHA-256 |
| --- | --- |
| SHA256SUMS | 9a6f1013eb01290344877cd62ae2787f9b4618449b73286d1d9c45860c606a3c |
| rust-mirror-0.6.0-aarch64-apple-darwin.tar.gz | d149ec6c2df2ae1ced34936142cd86f6ac0c9f31c0ea307eaffc78279b762d5b |
| rust-mirror-0.6.0-x86_64-apple-darwin.tar.gz | fe59e999e25d188c27deb9fd65e8b570024678ed197aee07d386612372f39616 |
| rust-mirror-0.6.0-x86_64-pc-windows-msvc.zip | 47764d58bd0c309c603254654b568f90e3132027ef5f7fd712548c88e41e57cf |
| rust-mirror-0.6.0-x86_64-unknown-linux-gnu.tar.gz | 28ae4a19678cd6e613864c542f630e0c54e77f1be6d2fdcfde4361ce55081d71 |
| rust-mirror-0.6.0-x86_64-unknown-linux-musl.tar.gz | 4a6ff890ebcfe5f3ff603896c1d45ff34f1941591f20db8a69e847a7010c8176 |
| rust-mirror-0.6.0.cdx.json | 90eb5cf32ea7bb081612d4ee8528b594519707a6d1bc5dd6cbfb8976f0ce8103 |
| rust-mirror-0.6.0.intoto.jsonl | 16f85dd17cf18f9920f0777a820028549bebc1f8efdbad83859dfeb852383b75 |

Independently verified on Linux/aarch64 with gh 2.100.0, using signer
rustcommons/rust-mirror/.github/workflows/build-binaries.yml, source and signer digest
`eaa6354…`, and `--deny-self-hosted-runners`:

- SLSA v1 provenance for all seven signed files (archives, SBOM, SHA256SUMS).
- The CycloneDX predicate for all five archives equals the downloaded SBOM and names
  each archive's digest. The SBOM's source commit and lock SHA-256 match the tag.
- `rust-mirror-0.6.0.intoto.jsonl` holds one Sigstore v0.3 bundle whose statement
  names exactly the seven signed files. `gh attestation verify --bundle` passes for
  all seven, and its DSSE payload and signature equal GitHub's stored attestation.
  The provenance names builder `build-binaries.yml@refs/tags/v0.6.0` and invocation
  run 44913249402.
- Every archive's README.txt names the source commit; LICENSE matches the tag.

## Public Windows, GNU and musl execution

[Public verifier 44913981641](https://github.com/rustcommons/rust-mirror/actions/runs/44913981641)
(reader revision = source eaa6354) passed on all three targets. Each job checked
the release body, all eight assets, every provenance, the SBOM binding and the
attached bundle against the store before executing its downloaded binary.

| Target | Binary SHA-256 | Result |
| --- | --- | --- |
| x86_64-pc-windows-msvc | 9f654f55480a1e2892a897ccf737874946b345f0f22ecaae88b9ee66a7cf5dab | 0.6.0, protocol 1; static CRT, no external CRT imports |
| x86_64-unknown-linux-gnu | 8fcde2c2c93165eb761079c74184fb7785a135de189e5b1355872ee016450480 | 0.6.0, protocol 1; max required GLIBC 2.17 |
| x86_64-unknown-linux-musl | 2903c47250f6944c27c6d93c7849830f4ebc4c3b24ef4e8c6c7e8c0fde56ec22 | 0.6.0, protocol 1; no interpreter or NEEDED libraries |

## Registry installation and docs.rs

A fresh `cargo install rust-mirror --version =0.6.0 --locked` (Cargo 1.90.0,
Linux/aarch64, new Cargo home/target/root, no registry token) fetched the crate with
the registry checksum. The installed binary reported 0.6.0/protocol 1, refused a
missing plan with typed `io`, and completed a real itoa 1.0.15 plan and approved
fetch (11231 bytes). At 2026-11-21T14:28:34Z docs.rs reported a successful build, and
the [API](https://docs.rs/rust-mirror/0.6.0/rust_mirror/),
[events](https://docs.rs/rust-mirror/0.6.0/rust_mirror/events/),
[crates](https://docs.rs/rust-mirror/0.6.0/rust_mirror/crates/) and
[lockfile](https://docs.rs/rust-mirror/0.6.0/rust_mirror/crates/lockfile/) pages
returned 200 for 0.6.0.

## History and limits

Releases v0.5.1–v0.5.4 are unchanged against the 0.5.4 record: release IDs, body
hashes, asset IDs, sizes and digests. v0.5.3 still has zero assets, and v0.5.4 has
no provenance bundle; none is backfilled. The library break (`Error::report` carries
toml 1's `Value`) is documented in the [0.6.0 notes](../releases/v0.6.0.md#upgrade).

GNU results cover container userlands on modern hosted kernels, not old native
kernels. macOS archives are cryptographically verified but not executed after
download, and Gatekeeper UI is untested. Windows/Apple executables are not OS
code-signed. The SBOM covers the locked Cargo graph only. L3 provenance is build
provenance, not an independent security certification. The
[machine-readable record](publication-0.6.0.json) holds the details. This record
changes documentation and the read-only verifier default only.
