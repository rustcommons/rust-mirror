# Security baseline evaluation — 2026-11-15

## Approved scope and live provider observations

The owner approved this baseline, not a release. The coordinator enabled provider
settings before the repository PR. Read-only API checks confirmed active protect-main
and protect-release-tags rulesets, configured CodeQL for Actions/Python/Rust, enabled
private vulnerability reporting and the crates-io environment's reviewer/branch-policy
rules with v* tag selection. No provider setting was changed here. The
[machine-readable observations](security-baseline-20261115.json) retain exact status
fields, upstream action commits, RustSec revision and dependency changes.

The source base is the normal merge of publication-record PR #48. Existing v0.5.1/
v0.5.2 source tags, registry payloads, assets and manifests are immutable. This work
uses one reviewable branch/PR; it does not publish software, mint a crates.io token
or add a SLSA3/Best Practices badge.

## Dependency findings and fixes

The initial cargo-deny0.20.2 scan found ten public RustSec TLS advisories:
RUSTSEC-2026-0049, 0098, 0099, 0104, 0285, 0346, 0348, 0349, 0381 and 0382
(each abbreviated suffix has the same RUSTSEC-2026 prefix). The subsequent full
feature/platform graph also reported RUSTSEC-2026-0486 in rustix1.1.2.
These are known upstream advisories, not a claim that every vulnerable optional
feature was active or an exploit was demonstrated in this CLI.

| Package | Original lock | Updated lock | Reason |
| --- | --- | --- | --- |
| rustls | 0.23.35 | 0.23.46 | TLS handshake/state/key and secret-Debug advisories |
| rustls-webpki | 0.103.8 | 0.103.17 | Certificate/constraint/CRL validation advisories |
| webpki-roots (parent of compatibility 0.26.11) | 1.0.4 | 1.0.10 | Removes the distrusted emSign root covered by 0381 |
| rustix | 1.1.2 | 1.1.6 | Zero-length xattr Buffer initialization/UB advisory 0486 |
| libc | 0.2.178 | 0.2.191 | Existing transitive dependency required by the rustix update |
| linux-raw-sys | 0.11.0 | 0.12.1 | Existing transitive dependency required by the rustix update |

No new crate names or registries were added. The resolver removed unused Windows
support entries; existing Windows dependencies still undergo native CI. Dependabot
PRs #49/#50 overlap the rustls/webpki updates and were left untouched for coordinator
disposition. The current source changes are unreleased: they do not fix already
published 0.5.2 archives. Shipping fixes requires a separate owner-approved version.

With deny.toml, the final local check passed advisories, bans, licenses and sources
with no ignored advisories. Current license alternatives are explicitly allowed;
remaining getrandom/syn major splits warn rather than force unrelated API migrations.
Yanks/vulnerabilities refuse, while all-graph informational unmaintained/unsound
notices stay visible for maintainer assessment. The RustSec DB revision is in JSON;
CI fetches current advisories instead of freezing the database indefinitely.

## Workflow audits and primary references

The initial zizmor1.32.0 audit found 72 unpinned uses and 13 persisted-checkout-token
findings. All 100 final direct third-party action uses have full commits/version
comments; credential persistence is disabled. Thirteen distinct action/version
references were resolved against live upstream GitHub commit APIs and matched their
pins. These are read-only production-provider cases, not invented resolution data.
Static actionlint1.7.12 and final local zizmor pass. Local Rust1.90 tests passed
191 cases including the doctest; all-target Clippy/rustdoc warnings-denied, fmt
and 55 Python tests passed. The only narrow inline notes
retain the documented same-commit ./ reusable spelling because actionlint does not
yet parse the newer $/ spelling. There is no global audit/severity bypass.

Design and source checks used primary documentation:

- [GitHub reusable builds and SLSA v1 Build L3](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/increase-security-rating): build and attest within the reusable boundary, validate its signer identity.
- [GitHub reusable syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax): both ./ and $/ refer to the caller commit.
- [crates.io authentication action](https://github.com/rust-lang/crates-io-auth-action): short-lived token output and post-job revocation.
- [Scorecard action](https://github.com/ossf/scorecard-action): main/schedule publishing with job-scoped OIDC and SARIF permissions.
- [cargo-deny configuration](https://embarkstudios.github.io/cargo-deny/checks/cfg.html) and [zizmor audits](https://docs.zizmor.sh/audits/).
- [Dependabot cooldown policy](https://docs.github.com/en/code-security/tutorials/secure-your-dependencies/optimizing-pr-creation-version-updates): security updates bypass version-update cooldowns.

## SBOM, isolation and negative controls

Pinned cargo-cyclonedx0.5.9 was installed with --locked and exercised on an exact
clean source worktree. Two disposable-source generations had identical canonical
SBOM bytes; their SHA-256s/source are in JSON. The helper checks every registry
component checksum against the untouched lock and records source commit/lock hash.
Only exact temporary package-ID prefixes and local purl download qualifiers are rewritten consistently; all references
and checksum identities remain. SOURCE_DATE_EPOCH fixes metadata time. This is a
Cargo all-target graph, not a complete OS/compiler/CI-tool SBOM.

Tests exercise wrong SBOM format/root/graph, reference canonicalization, wrong
archive/SBOM predicate/digest bindings, tampered/dirty/extra registry payloads,
checksum/yank refusals, read-only already-published/missing-version paths, and a
comparator that refuses to label changed executable bytes as identity. Windows release RUSTFLAGS add /Brepro alongside static CRT; its actual effect is
measured in the independent runs, not assumed. PE timestamp
normalization is diagnosis only; actual hashes and nonidentity remain recorded.
The known published v0.5.2 read-only verifier matched its real API/index/archive,
clean tag VCS and original payload without token exchange or upload.

The first hosted Windows run exposed fixture-only CRLF/URI assumptions; fixture
payloads now come from frozen Git objects and URI cases use explicit POSIX paths.
CodeQL flagged default-branch cache scope for caller-selected checkout refs; source
execution is now bound to the invocation SHA instead of input refs. The test HTTP
stub also uses exact URL keys, avoiding misleading substring sanitization. Earlier
failed checks remain in PR history; subsequent exact-head checks qualify the fixes.

PR/repro callers invoke the read-only shared core, not a workflow declaring a
signing-capable job. Production calls the reusable build/sign wrapper, whose core
requires own-repository main-reachable immutable source, correct event/ref and
strict empty-release preflight before signing. Native/core jobs explicitly downgrade
to contents read. The isolated signer has OIDC/attestation writes and the separate
uploader contents write only. Every archive must have both provenance and the exact
SBOM binding verified before upload. Existing assets refuse; no replacement path.

## Qualification outcomes and remaining evidence

The PR's security, native/core, SBOM and reproducibility workflow runs supply the
exact-head execution evidence. The reproducibility workflow emits five-target
Markdown/JSON results from two fresh jobs per target; copy the actual outcome table
and run IDs to the PR review record after it completes. This document makes no
advance bit-identity assertion. Hash differences must retain observed causes or
explicitly unresolved root causes; green measurement CI is not a bit-identity badge.

New live SBOM/provenance and the L3 signer identity cannot be verified under this
no-publication task; only a future approved release can close that gap. Trusted
Publishing also needs the owner's exact crates.io registration and an actual
approved publication. The existing manual process remains; no token exchange is
attempted here. Scorecard's first new main-only result follows merge. All three
constraints are documented, not silently treated as completed production evidence.
See [the operational guide](../SECURITY-BASELINE.md) for job privileges, profile
compatibility, exact owner settings and future verification commands.
