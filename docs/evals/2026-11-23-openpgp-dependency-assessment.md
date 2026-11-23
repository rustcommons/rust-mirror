# OpenPGP dependency and trust-anchor assessment — 2026-11-23

## Status

The owner authorized rPGP (`pgp`) and the pinned Rust release-key policy on
2026-11-22. This is an initial assessment, not a completed feature, release
qualification or claim that existing mirror operations authenticate manifests.
The prototype compiles on Rust 1.90 and verifies four real manifests, but the
required cargo-deny gate is blocked. No ignore, license exception, version bump,
feature PR, merge, tag or publication has been made for this feature.
The work is preserved on [feat/openpgp-manifests](https://github.com/rustcommons/rust-mirror/tree/feat/openpgp-manifests)
without a PR; the branch remains blocked and is not ready to merge.

## Official key cross-check

The independently computed V4 primary fingerprint is
`108F66205EAEB0AAA8DD5E1C85AB96E6FA1BE5FE` at all three official locations:

- [Official distribution key](https://static.rust-lang.org/rust-key.gpg.ascii).
- [Rustup 1.25.1 embedded key at its source commit](https://raw.githubusercontent.com/rust-lang/rustup/bb60b1e89cc01a682dd16f2854329030938d3fe3/src/rust-key.pgp.ascii).
- [Archived Rust website key at its source commit](https://raw.githubusercontent.com/rust-lang/prev.rust-lang.org/3c8d24f62264c5dd1bd8cb4acc29c2fa2bbc678e/rust-key.gpg.ascii).

Checked on 2026-11-23. Each armor is 5,326 bytes, SHA-256
`e54b09a439647e006b4831eec9785cbaaf3e07ab371c3a6ee6a68e1bdb9fbc6b`.
A separate standard-library packet reader derived the fingerprint from the V4
primary packet (SHA-1 of the OpenPGP fingerprint framing); rPGP independently
computed the same fingerprint. This SHA-1 fingerprint calculation is the key
identity format, not permission to trust weak manifest signature hashes.
Current Rustup main no longer contains the historical embedded key, so the
source link deliberately pins the older official release rather than claiming
it is present in current Rustup.

## Dependency choice and audit refusal

`pgp` 0.21.1 is the current published release inspected. Its declared MSRV is
1.88; `cargo +1.90.0 check --lib -j 2` passes. `default-features = false` disables
optional bzip2; no optional feature is enabled. rPGP's other encryption/signing
algorithms remain unconditional dependencies and cannot be removed through its
published feature flags.

Pinned cargo-deny 0.20.2, with the repository's unchanged deny.toml, reports
**advisories FAILED, bans ok, licenses ok, sources ok**:

| Crate | Refusal | Patched range | Why a lock update alone cannot fix it |
| --- | --- | --- | --- |
| rsa 0.9.10 | [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071), private-key timing leakage | >=0.10.0 | pgp requires ^0.9.10; a precise update to 0.10.0 fails resolution. |
| crypto-bigint 0.5.5 | [RUSTSEC-2026-0387](https://rustsec.org/advisories/RUSTSEC-2026-0387), secret-dependent arithmetic timing | >=0.7.6 | cx448/elliptic-curve require ^0.5.5; updating to the patched major range fails resolution. The first patched 0.7.6 release is yanked; any future resolution should choose a non-yanked patched version. |

The experimental verifier only uses public certificates and public signature
verification; it loads no secret keys and performs no signing/decryption. That
bounded exposure assessment does not override the dependency-based audit gate.
No advisory has been ignored. Owner decision, 2026-11-22, **option B**: grant
NO advisory exceptions and wait for a released rPGP dependency graph that clears
both findings. The owner's approval of `pgp` and the pinned-key trust policy
still stands. Updating `pgp` and obtaining a clean cargo-deny result with no
exceptions is mandatory before resuming the feature PR; also recheck MSRV 1.90,
licenses and the newly resolved transitive graph. A patched numeric range alone
is insufficient if the resolved version is yanked or another advisory remains.

Existing upstream tracking, checked 2026-11-23: [rPGP PR #573, Adopt RustCrypto
new releases](https://github.com/rpgp/rpgp/pull/573), is open; its inspected diff
proposes `rsa` 0.10 and `crypto-bigint` 0.7.11. [Issue #257](https://github.com/rpgp/rpgp/issues/257)
tracks RUSTSEC-2023-0071. These are monitoring references, not evidence that a
published rPGP version is already qualified. No upstream issue or PR was opened.

## Preliminary real-data verification

A local Rust probe verified the original `.toml` bytes and corresponding `.asc`
for official `1.90.0`, `stable`, `beta` and `nightly` endpoints. Each verified
under the expected primary fingerprint, with authenticated signature timestamps.
These four positive cases establish initial compatibility only. The complete
negative/time/revocation/refresh, old-cache/resume, fuzz and 10–20+ case live
qualification remain unfinished; no end-to-end product behavior is claimed.
The temporary probe was removed from the shipped examples.

## Added dependency versions and licenses

The following table is generated from locked Cargo metadata relative to main.
It includes newly introduced versions of existing transitive crates as well as
new crate names, so existing version updates are not hidden. `pgp` is the sole
new direct dependency; all other rows are transitive. SPDX alternatives satisfy
the existing license allowlist; the audit reported licenses ok with no exception.
See the [machine-readable inventory](openpgp-dependencies-20261123.json) for
new-name flags, MSRVs and the exact baseline.

| Crate | Version | Declared license |
| --- | --- | --- |
| aead | 0.5.2 | MIT OR Apache-2.0 |
| aes | 0.8.4 | MIT OR Apache-2.0 |
| aes-gcm | 0.10.3 | Apache-2.0 OR MIT |
| aes-kw | 0.2.1 | MIT OR Apache-2.0 |
| argon2 | 0.5.3 | MIT OR Apache-2.0 |
| base16ct | 0.2.0 | Apache-2.0 OR MIT |
| base64ct | 1.8.3 | Apache-2.0 OR MIT |
| bitfields | 1.0.3 | MIT |
| bitfields-impl | 1.0.3 | MIT |
| bitvec | 1.1.1 | MIT |
| blake2 | 0.10.6 | MIT OR Apache-2.0 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 |
| block-padding | 0.3.3 | MIT OR Apache-2.0 |
| blowfish | 0.9.1 | MIT OR Apache-2.0 |
| buffer-redux | 1.1.0 | MIT OR Apache-2.0 |
| byteorder | 1.5.0 | Unlicense OR MIT |
| bytes | 1.12.2 | MIT |
| camellia | 0.1.0 | MIT OR Apache-2.0 |
| cast5 | 0.11.1 | MIT OR Apache-2.0 |
| cfb-mode | 0.8.2 | MIT OR Apache-2.0 |
| cipher | 0.4.4 | MIT OR Apache-2.0 |
| cmac | 0.7.2 | MIT OR Apache-2.0 |
| const-oid | 0.9.6 | Apache-2.0 OR MIT |
| convert_case | 0.10.0 | MIT |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 |
| crc24 | 0.1.6 | MIT/Apache-2.0 |
| crypto-bigint | 0.5.5 | Apache-2.0 OR MIT |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 |
| ctr | 0.9.2 | MIT OR Apache-2.0 |
| curve25519-dalek | 4.1.3 | BSD-3-Clause |
| curve25519-dalek-derive | 0.1.1 | MIT/Apache-2.0 |
| cx448 | 0.1.1 | BSD-3-Clause |
| darling | 0.20.11 | MIT |
| darling_core | 0.20.11 | MIT |
| darling_macro | 0.20.11 | MIT |
| dbl | 0.3.2 | MIT OR Apache-2.0 |
| der | 0.7.10 | Apache-2.0 OR MIT |
| derive_builder | 0.20.2 | MIT OR Apache-2.0 |
| derive_builder_core | 0.20.2 | MIT OR Apache-2.0 |
| derive_builder_macro | 0.20.2 | MIT OR Apache-2.0 |
| derive_more | 2.1.1 | MIT |
| derive_more-impl | 2.1.1 | MIT |
| des | 0.8.1 | MIT OR Apache-2.0 |
| digest | 0.10.7 | MIT OR Apache-2.0 |
| dsa | 0.6.3 | Apache-2.0 OR MIT |
| eax | 0.5.0 | Apache-2.0 OR MIT |
| ecdsa | 0.16.9 | Apache-2.0 OR MIT |
| ed25519 | 2.2.3 | Apache-2.0 OR MIT |
| ed25519-dalek | 2.2.0 | BSD-3-Clause |
| elliptic-curve | 0.13.8 | Apache-2.0 OR MIT |
| ff | 0.13.1 | MIT/Apache-2.0 |
| fiat-crypto | 0.2.9 | MIT OR Apache-2.0 OR BSD-1-Clause |
| fnv | 1.0.7 | Apache-2.0 / MIT |
| funty | 2.0.0 | MIT |
| generic-array | 0.14.7 | MIT |
| ghash | 0.5.1 | Apache-2.0 OR MIT |
| group | 0.13.0 | MIT/Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| hkdf | 0.12.4 | MIT OR Apache-2.0 |
| hmac | 0.12.1 | MIT OR Apache-2.0 |
| idea | 0.5.1 | MIT OR Apache-2.0 |
| ident_case | 1.0.1 | MIT/Apache-2.0 |
| inout | 0.1.4 | MIT OR Apache-2.0 |
| k256 | 0.13.4 | Apache-2.0 OR MIT |
| keccak | 0.1.6 | Apache-2.0 OR MIT |
| libm | 0.2.16 | MIT |
| libz-rs-sys | 0.5.5 | Zlib |
| md-5 | 0.10.6 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| nom | 8.0.0 | MIT |
| num-bigint-dig | 0.8.6 | MIT/Apache-2.0 |
| num-iter | 0.1.46 | MIT OR Apache-2.0 |
| num_enum | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 |
| num_enum_derive | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 |
| ocb3 | 0.1.0 | Apache-2.0 OR MIT |
| opaque-debug | 0.3.1 | MIT OR Apache-2.0 |
| p256 | 0.13.2 | Apache-2.0 OR MIT |
| p384 | 0.13.1 | Apache-2.0 OR MIT |
| p521 | 0.13.3 | Apache-2.0 OR MIT |
| password-hash | 0.5.0 | MIT OR Apache-2.0 |
| pem-rfc7468 | 0.7.0 | Apache-2.0 OR MIT |
| pgp | 0.21.1 | MIT OR Apache-2.0 |
| pkcs1 | 0.7.5 | Apache-2.0 OR MIT |
| pkcs8 | 0.10.2 | Apache-2.0 OR MIT |
| polyval | 0.6.2 | Apache-2.0 OR MIT |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 |
| primeorder | 0.13.6 | Apache-2.0 OR MIT |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| radium | 0.7.0 | MIT |
| rand | 0.8.8 | MIT OR Apache-2.0 |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 |
| rand_core | 0.6.4 | MIT OR Apache-2.0 |
| replace_with | 0.1.8 | MIT OR Apache-2.0 |
| rfc6979 | 0.4.0 | Apache-2.0 OR MIT |
| ripemd | 0.1.3 | MIT OR Apache-2.0 |
| rsa | 0.9.10 | MIT OR Apache-2.0 |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 |
| rustversion | 1.0.23 | MIT OR Apache-2.0 |
| sec1 | 0.7.3 | Apache-2.0 OR MIT |
| serdect | 0.2.0 | Apache-2.0 OR MIT |
| serdect | 0.3.0 | Apache-2.0 OR MIT |
| sha1 | 0.10.7 | MIT OR Apache-2.0 |
| sha1-checked | 0.10.0 | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| sha3 | 0.10.9 | MIT OR Apache-2.0 |
| signature | 2.2.0 | Apache-2.0 OR MIT |
| snafu | 0.9.2 | MIT OR Apache-2.0 |
| snafu-derive | 0.9.2 | MIT OR Apache-2.0 |
| spin | 0.9.9 | MIT |
| spki | 0.7.3 | Apache-2.0 OR MIT |
| strsim | 0.11.1 | MIT |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| tap | 1.0.1 | MIT |
| twofish | 0.7.1 | MIT OR Apache-2.0 |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 |
| unicode-xid | 0.2.6 | MIT OR Apache-2.0 |
| universal-hash | 0.5.1 | MIT OR Apache-2.0 |
| version_check | 0.9.5 | MIT/Apache-2.0 |
| wyz | 0.5.1 | MIT |
| x25519-dalek | 2.0.1 | BSD-3-Clause |
| zerocopy | 0.8.64 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.64 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zeroize_derive | 1.5.0 | Apache-2.0 OR MIT |
| zlib-rs | 0.5.5 | Zlib |
