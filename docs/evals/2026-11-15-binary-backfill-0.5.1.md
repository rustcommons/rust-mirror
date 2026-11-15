# rust-mirror 0.5.1 binary backfill — 2026-11-15

## Authorization and immutable source

The owner authorized unsigned GitHub Release binaries and the current-version
backfill. The coordinator reviewed/normal-merged
[PR #43](https://github.com/rustcommons/rust-mirror/pull/43): main
`cb9833a8bd18cf71f17c6e9d5cc0962ae39f7cfd` is tree-identical to qualified
`51eb1686d4383b37772de1bd8477c90570e91e98`. The coordinator then explicitly directed
the non-dry workflow_dispatch from main. The earlier PR/dispatch dry runs remain
qualification evidence; an additional postmerge dry run was not performed.

The [live backfill run](https://github.com/rustcommons/rust-mirror/actions/runs/43693076759)
passed all four builds, original/extracted smokes, collection and the privileged
attestation/upload job. It built immutable `v0.5.1` source
`112ef7a7a71d40349c601f299f965199b5b279ba` using Rust 1.90.0 and --locked.
There was no version bump, crates.io publication, tag move or release-note edit.
The [initial publication record](2026-11-15-publication-0.5.1.md) remains historical;
its crates.io archive SHA-256
`29b547bb8b762beb0f403dda6f8f628ab7706a18ccd52bb0f01711382a0bda39` is unchanged.

## Live assets and attestations

The [existing release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.5.1)
now contains exactly four archives plus SHA256SUMS; it had zero assets before this
backfill. No asset was replaced or deleted. Its body remains byte-identical to
both the pre-backfill snapshot and the tagged notes.

| Asset | SHA-256 |
| --- | --- |
| [Windows x86-64 ZIP](https://github.com/rustcommons/rust-mirror/releases/download/v0.5.1/rust-mirror-0.5.1-x86_64-pc-windows-msvc.zip) | 0b2293a4a424c6fa42724d6bf9f6188ae24650859d38537568e2ba4d5bd71a7c |
| [Linux x86-64 static musl](https://github.com/rustcommons/rust-mirror/releases/download/v0.5.1/rust-mirror-0.5.1-x86_64-unknown-linux-musl.tar.gz) | d79e96a285edd6c62c6c54b83ba08478669f8a9ec831a637f2124f1e274cdb19 |
| [macOS Apple Silicon](https://github.com/rustcommons/rust-mirror/releases/download/v0.5.1/rust-mirror-0.5.1-aarch64-apple-darwin.tar.gz) | a07b4359ad1ed16e32ab49f50b6be7c3ebd1ca95fe8cf4cbb7aed1223ef6827b |
| [macOS Intel](https://github.com/rustcommons/rust-mirror/releases/download/v0.5.1/rust-mirror-0.5.1-x86_64-apple-darwin.tar.gz) | c95f884266694e9e1ba143bc1f2be5913081404dceb8229205322a5a1742ed10 |
| [SHA256SUMS](https://github.com/rustcommons/rust-mirror/releases/download/v0.5.1/SHA256SUMS) | 431f5f732f48ad8310f94ebc61d92eb271f38fce9557d1dd52ef6862433bb171 |

Downloaded all five public assets independently. Every hash matches GitHub's asset
metadata; SHA256SUMS contains exactly four sorted archive entries with matching
hashes. gh attestation verify passed for every archive and SHA256SUMS, scoped to
rustcommons/rust-mirror and .github/workflows/release-binaries.yml. The publish job
also verified all attestations before uploading. Signed statements identify reviewed
main workflow revision cb9833a; the attested archive README binds separate source
commit 112ef7a. Do not equate workflow and source revisions for this backfill.
[Machine-readable evidence](binary-backfill-0.5.1.json) preserves hashes/results.

## Actual downloaded Windows and Linux execution

[Read-only native verification](https://github.com/rustcommons/rust-mirror/actions/runs/43693774323)
passed on windows-latest and ubuntu-22.04. Each job downloaded the actual public
assets, verified every checksum/attestation and tagged license/source record, then
ran its native extracted executable. Both reported rust-mirror 0.5.1 and protocol 1.
No Rust rebuild/install was used for these public-download execution checks.
Workflow/API credentials were removed from the product process environment.

The downloaded Windows ZIP has only normal OS DLL imports: KERNEL32, bcryptprimitives,
api-ms-win-core-synch, bcrypt, ADVAPI32, ntdll and WS2_32 (case variants are preserved
in the JSON). No vcruntime/msvcp/ucrtbase/api-ms-win-crt imports remain. The reviewed
release build alone uses static CRT; ordinary Cargo/dev settings remain unchanged.
The executable therefore does not require a separate Visual C++ Redistributable.

The downloaded Linux file reports static-pie linked; ldd reports statically linked.
ELF headers have no INTERP or NEEDED entries. The backfill build also ran the same
binary in an empty chroot containing no loader/libc/OpenSSL/NSS modules. There is
no glibc runtime floor. Existing locked dependencies compiled without changes;
musl-tools was a build-runner C compiler prerequisite, not a shipped dependency.

## DNS/TLS, compatibility and historical bounds

The musl executable completed real itoa preview/whole-plan approval/sync through
index.crates.io and static.crates.io over verified HTTPS with bundled rustls roots.
The private-CA CONNECT fixture passed killed transfer, stdin cancellation, exact
resume, retry and heartbeat assertions. Both /etc/hosts and /etc/resolv.conf were
available; musl uses its resolver rather than glibc NSS plugins. This is named
operational evidence, not DNS packet inspection or every SSSD/mDNS/proxy policy.
The separate isolated Cargo HTTPS client controls retain their own scope.

All four native build runners executed their original and unpacked binaries;
post-publication public-download execution was Windows/Linux only. macOS public
archives were hash/provenance checked. No browser-generated SmartScreen/Gatekeeper
quarantine UI, every legacy kernel/CPU, production Nexus or arbitrary enterprise
configuration was qualified. No paid Authenticode/Developer ID signing or notarization
was added; GitHub/Sigstore provenance is a separate signature.

The initial GNU/glibc 2.35 design and dynamic-CRT dry runs were superseded before
attachment; they were never public release assets. Preserve their historical PR
comments and canceled intermediate runs. Current live artifacts use musl/static CRT.
The original release notes and crates.io payload are immutable and unchanged.

The reviewed [main CI](https://github.com/rustcommons/rust-mirror/actions/runs/43693060733)
passed at workflow merge cb9833a. This follow-up records evidence and adds the
repeatable read-only public-download verifier; it does not alter the release builder,
source/versions/dependencies, notes, tag or assets and requires coordinator review.
