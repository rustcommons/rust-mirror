# rust-mirror 0.6.1 publication — 2026-11-22

## Source and first phase 2 run

The owner merged [PR #79](https://github.com/rustcommons/rust-mirror/pull/79) at
`3c11d914bce9a06cb92ef7f39179e3a42f98cab0`. The merge tree equals qualified PR head
`641242b7f878deb1665215cfbee0a19055b76bb7`; its full candidate dry run
[44988751765](https://github.com/rustcommons/rust-mirror/actions/runs/44988751765)
passed. Annotated tag `v0.6.1` (object
`f49831939f907a9854ce7087571e5cc8550c08fa`) peels to the merge. The four patch fixes
and additive API/serialization compatibility qualification are recorded in the
[candidate evaluation](2026-11-22-release-0.6.1.md) and
[release notes](../releases/v0.6.1.md).

[Trusted publishing 44992707987, attempt 2](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/attempts/2)
completed successfully. This is the first real release through phase 2: one tag run
built the crate and five archives, signed before owner approval, published to
crates.io after approval, created the GitHub Release and verified public downloads.
No separate release-binaries dispatch or manual release creation was needed.

## Initial gate refusal and unchanged-tag rerun

[Attempt 1](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/attempts/1)
failed at 06:13:12 UTC with:

> no successful CI run for 3c11d914bce9a06cb92ef7f39179e3a42f98cab0; wait for CI on main, then re-run

The tag was pushed before CI on the merge commit finished. Build, publish,
GitHub release and verify were skipped: that attempt neither signed nor published.
After CI finished, the same run was rerun against the same source/tag and passed.
This is the intended gate refusal, not a bypass or a changed signer policy. Future
release tags must wait for green CI on the actual merge commit, even when the PR
head was already green. The initial failed attempt remains part of the record.

## First real behavior of each job

All times below are UTC on 2026-11-22; the table is the successful second attempt.

| Job | Start | Finish | Outcome |
| --- | --- | --- | --- |
| [Release gate](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137118126123) | 06:17:32 | 06:17:48 | success |
| [Build / binaries / validate](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137118196925) | 06:17:50 | 06:17:55 | success |
| [Build / binaries / crate-sbom](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137118223376) | 06:17:57 | 06:21:20 | success |
| [Build / binaries / build (ubuntu-24.04, x86_64-unknown-linux-gnu)](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137118223402) | 06:17:57 | 06:20:33 | success |
| [Build / binaries / build (windows-2025, x86_64-pc-windows-msvc)](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137118223439) | 06:17:57 | 06:20:46 | success |
| [Build / binaries / build (macos-15, aarch64-apple-darwin)](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137118223454) | 06:17:58 | 06:20:12 | success |
| [Build / binaries / build (ubuntu-22.04, x86_64-unknown-linux-musl)](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137118223503) | 06:17:57 | 06:20:31 | success |
| [Build / binaries / build (macos-15-intel, x86_64-apple-darwin)](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137118223528) | 06:18:02 | 06:22:32 | success |
| [Build / binaries / verify](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137119284725) | 06:22:35 | 06:22:48 | success |
| [Build / Checksums and attestations](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137119342136) | 06:22:50 | 06:23:02 | success |
| [Publish to crates.io](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137119392668) | 13:46:53 | 13:47:12 | success |
| [GitHub release](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137229136668) | 13:47:15 | 13:48:44 | success |
| [Verify publication (ubuntu-22.04, x86_64-unknown-linux-gnu)](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137229532458) | 13:48:46 | 13:51:31 | success |
| [Verify publication (ubuntu-22.04, x86_64-unknown-linux-musl)](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137229532463) | 13:48:47 | 13:50:10 | success |
| [Verify publication (windows-2025, x86_64-pc-windows-msvc)](https://github.com/rustcommons/rust-mirror/actions/runs/44992707987/job/137229532485) | 13:48:46 | 13:50:15 | success |

- **Gate:** accepted the exact tag/main source after successful main CI and the
  single expected crate; no registration identity or protection was changed.
- **Read-only build core:** validated the frozen source, generated the exact-lock
  all-target CycloneDX SBOM, packaged/audited the 45-member crate, and installed
  that package cleanly. Five native builds passed, including all eight GNU
  userland gates, musl network/resume qualification and Windows static CRT checks.
- **Collector (`Build / binaries / verify`):** downloaded the qualified outputs,
  rechecked the exact file set and hashes and produced one seven-entry SHA256SUMS
  covering the crate, five archives and SBOM. It did not wait for a public release
  or run Rust/third-party build tools; the old 0.6.0 collector ordering timeout
  did not recur.
- **Isolated signer (`Build / Checksums and attestations`):** revalidated the tag
  and hashes, generated SLSA v1 provenance for all eight signed files (the seven
  checksum subjects plus SHA256SUMS), staged the single provenance bundle, and
  bound the same CycloneDX document to the five archives and crate. Signing
  finished at 06:23:02, before the publishing environment admitted the owner-approved
  publish job at 13:46:53. Approval timing therefore did not hold up signing.
  The log contains the `actions/attest-sbom` deprecation warning; it did not prevent
  signing. Its replacement belongs to separate maintenance, not this immutable tag.
- **Publish:** rechecked SHA256SUMS and registry absence after owner approval;
  `cargo package --locked --no-verify` and `cmp` matched the attested crate before
  token minting. The Trusted Publishing action supplied the short-lived token to
  `cargo publish --locked --no-verify`. Both packaging calls avoided compilation;
  no crate/dependency code compiled in the OIDC-enabled publish job. This was a
  real upload, not an already-published skip. crates.io recorded publication at
  `2026-11-22T13:47:08.586852Z`.
- **GitHub release:** verified the attached bundle/store match and SBOM bindings,
  created draft release 454479811 at 13:48:31, uploaded all nine assets, checked the
  complete set, then published it at 2026-11-22T13:48:40Z. The live release is
  public, not draft, not prerelease, and was marked latest. Its body equals the
  committed versioned notes. No existing asset replacement/recovery was exercised.
- **Verify:** all GNU, musl and Windows jobs downloaded the public assets, verified
  hashes, exact-source/signer provenance, attached bundle/store equality, SBOM
  bindings and source/license records before executing their native downloads.
  The GNU job additionally checked registry/tag payload equality, the docs.rs
  build and a fresh locked registry installation. These extra steps are deliberately
  skipped on musl/Windows; their public native verification still ran and passed.

## Registry, docs.rs and installation

crates.io records the GitHub Trusted Publisher as `rustcommons/rust-mirror`, run
`44992707987`, source `3c11d914bce9a06cb92ef7f39179e3a42f98cab0`. Version 0.6.1 is not yanked and
requires Rust 1.90. API, sparse index, downloaded crate and GitHub `.crate` asset
agree on SHA-256 `6a658865f500d18ca6e41c244425c09008c03025a6ac5a90db7622d3c20f6d70`.

Independent `python3 scripts/verify_registry_release.py --tag v0.6.1` passed:
clean tag VCS identity, exact allowed archive members, original source bytes and
normalized manifest/dependency policy. The build's package audit passed with 45
regular members. The release's attested crate is exactly the published registry
crate; the PR candidate archive is not substituted for it.

The GNU publication-verifier log reports `docs.rs built rust-mirror 0.6.1` at
13:50:06 and a successful fresh `cargo install rust-mirror --version =0.6.1 --locked`
under a new Cargo home/install root at 13:51:29, followed by a passing 0.6.1/protocol 1
hello check. This is actual registry installation, separate from the build job's
packaged-source install. A later read-only check found a successful docs.rs build
and HTTP 200 for the [API](https://docs.rs/rust-mirror/0.6.1/rust_mirror/) and
[crates API](https://docs.rs/rust-mirror/0.6.1/rust_mirror/crates/). No second local
registry rebuild is claimed.

## Nine live assets and independent cryptographic check

The [GitHub Release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.6.1)
has exactly the following nine uploaded files. SHA256SUMS contains seven sorted
entries; it excludes itself and the attached bundle.

| Asset | SHA-256 |
| --- | --- |
| SHA256SUMS | 78dbb0ea9f80f0d0cbfaef2a06baa60c96fc401c067ca80aab8b253624dff294 |
| rust-mirror-0.6.1-aarch64-apple-darwin.tar.gz | 6371f76baa02a0ee2dc54dcb5453b69a05d87b8e5447a443d89b8f29c989da44 |
| rust-mirror-0.6.1-x86_64-apple-darwin.tar.gz | d12d7648c8d7dff15bbb37de130733e6d4c3ef633ccca667f93d6f59e631b237 |
| rust-mirror-0.6.1-x86_64-pc-windows-msvc.zip | 9cbba7a524f754b89b466d2871511028e1e15084c5c36397831c1750d0b0de35 |
| rust-mirror-0.6.1-x86_64-unknown-linux-gnu.tar.gz | 2ab3827678ec89d4a8ca93d1d4ca90491c781f71342bc9b6aadb9a3854daf902 |
| rust-mirror-0.6.1-x86_64-unknown-linux-musl.tar.gz | 57a9c16025d855a1073aa862f97ec398f56735544442880c1abb6cfacb583e53 |
| rust-mirror-0.6.1.cdx.json | 16e45db7e54f1651116ea24d53b4e67c2a48c58c9fb9e035f8e9f36a663e15db |
| rust-mirror-0.6.1.crate | 6a658865f500d18ca6e41c244425c09008c03025a6ac5a90db7622d3c20f6d70 |
| rust-mirror-0.6.1.intoto.jsonl | 2919261a6c80cb489c81983e6b18d6aeb508c2da590b0f7cbbfa6e3ab8bcb8b4 |

An independent read-only audit downloaded all nine files and checked every GitHub
asset digest and checksum entry, exact release body, all eight attached-bundle
provenances and equality with GitHub's stored attestations. It also verified all
six CycloneDX bindings (five archives plus the crate); each predicate equals the
downloaded SBOM and binds the subject digest. The SBOM source commit/lock checksum
match the tag. Policy remained signer workflow
`rustcommons/rust-mirror/.github/workflows/build-binaries.yml`, exact source and
signer digest `3c11d914bce9a06cb92ef7f39179e3a42f98cab0`, and `--deny-self-hosted-runners`.
The predicates remain `https://slsa.dev/provenance/v1` and
`https://cyclonedx.org/bom`. No source/signer exception or action-name pin was used.

## Public native execution

The three saved `publication-proof-*` reports from the successful tag run identify
0.6.1/protocol 1, unchanged release notes, matching checksums and bundle/store proof.
Execution evidence below is from those hosted jobs, not a claimed local rerun.

| Target | Binary SHA-256 | Linkage check |
| --- | --- | --- |
| x86_64-pc-windows-msvc | af9997d1de95675def39c86c1fc5582bf388e227ff3034bc7ff6ebc2ba3fc6be | Static CRT; no external CRT imports |
| x86_64-unknown-linux-gnu | a8d62f1469bc3c6679ed487bc88ee2e74b23fbd4fe0019bbbb610c29e7127d6a | Maximum required GLIBC 2.17 |
| x86_64-unknown-linux-musl | 5fd92438e450bce13f864a9097ea1cf57e0340e6bf2bee18c5df85401ff2c35d | Static PIE; no interpreter or NEEDED libraries |

## Limits and follow-up

This record adds evidence; it changes no application, version, lockfile, workflow,
verifier policy, immutable tag, published asset or historical release record.
macOS archives were built/smoked and cryptographically checked; public-download
macOS execution and Gatekeeper UI were not qualified. GNU container tests use
modern hosted kernels, not old native kernels. Executables are not OS code-signed;
the SBOM covers the locked Cargo graph, not the whole build host. Provenance is
not an independent security certification. Product regression and filesystem/
network limits from the candidate evaluation still apply.

The release-time checks passed. Subsequent maintenance PR #80 exposed an inherited
`url` 2.5.7 advisory (RUSTSEC-2026-0592); the release retains its immutable lockfile.
Recording successful publication does not waive that later cargo-deny failure or
claim the release is free of advisories. Attestation-action maintenance and any
security dependency update remain separate work.

See the [machine-readable record](publication-0.6.1.json) for source, run/job,
registry, asset and platform identities. The GUI maintainer is notified after this
publication record PR is opened.
