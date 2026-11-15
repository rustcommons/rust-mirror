# Repository security automation

The organization security policy and private reporting route are maintained in
[rustcommons/.github](https://github.com/rustcommons/.github/security/policy).
This guide describes this repository's build/dependency automation, not a separate
disclosure policy or an assertion that all software is vulnerability-free.

## Source and dependency gates

All main changes require a PR, including administrative maintenance. Preserve the
required lf-only check, reviewable commits and other CI checks. Never force-push or
delete main; protected v* tags cannot be updated or deleted. Do not use the ruleset's
administrative bypass as a normal workflow. The coordinator enabled the provider
settings on 2026-11-15; this PR does not alter repository settings.

Dependabot checks Cargo and Actions weekly, groups minor/patch version updates,
limits each ecosystem to five open version-update PRs, and uses a seven-day version cooldown.
GitHub security updates bypass that cooldown and schedule. All direct third-party
actions use full commits with version comments. Checkout credentials are not
persisted. Top-level tokens are read-only; job privileges are listed below.

PRs run [zizmor](../.github/workflows/security.yml) 1.32.0 and cargo-deny 0.20.2.
[deny.toml](../deny.toml) checks the full cross-platform feature graph: RustSec
vulnerabilities and yanks refuse, unmaintained/unsound reports remain visible across
all dependencies as informational warnings pending maintainer assessment, licenses use the explicit current-graph allowlist, duplicate
versions warn, and non-crates.io/git dependency sources refuse. There are no ignored
advisories. The remaining getrandom/syn splits are transitive major versions, with
warning rather than unrelated API migration. Recheck changing upstream findings.

OpenSSF Scorecard runs on main pushes and weekly, publishes results and uploads
SARIF. Its badge is not a promised score; the first new workflow run follows merge.
CodeQL default setup and organization security features remain provider-managed.
OpenSSF Best Practices registration is handled by the owner; no badge is fabricated.

## Release privilege boundaries

| Job/path | Token permissions beyond contents read | Guard |
| --- | --- | --- |
| Native build, tests, SBOM generation, comparison | None | Frozen commit; no signing token or upload authority |
| Reusable build-binaries.yml attestation | id-token write, attestations write | Completed reviewed-source build; main-reachable immutable tag; execution SHA equals invocation SHA; no candidate signing |
| Release asset upload | contents write | Exact tag/notes, all attestations verified; refuses any existing asset |
| crates.io publish | id-token write | v* tag, main ancestry, crates-io environment and owner review |
| Scorecard | id-token write, security-events write | Main push or schedule only; no PR code |

Build execution is fixed to the invoking PR/branch/tag SHA, never an arbitrary
input ref in a main-scoped job. Dispatch a future historical release against its
exact immutable version tag; unrelated main revisions cannot build alternate refs.

The reusable [build-binaries.yml](../.github/workflows/build-binaries.yml) calls
[the shared read-only core](../.github/workflows/build-binaries-core.yml), then
attests inside the reusable boundary. Dry-run/repro callers use the same core with
no signing-capable job; native/core jobs explicitly downgrade to contents read.
Only the isolated attestation job can mint a signing identity; the outer uploader
has no OIDC permission. Reproducibility jobs never attest or upload release assets.

This follows [GitHub's reusable-workflow SLSA Build L3 design](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/increase-security-rating).
It does not upgrade existing v0.5.1/v0.5.2 provenance. There is no SLSA3 badge until
an owner-approved future release actually produces and verifies the new provenance.
For that profile, verify with signer workflow
rustcommons/rust-mirror/.github/workflows/build-binaries.yml and require both
--source-digest and --signer-digest to equal the independently inspected immutable
release commit; refuse self-hosted runners. The automated upload/download verifiers
enforce these checks ([CLI policy reference](https://cli.github.com/manual/gh_attestation_verify)). Historical releases
still require their original release-binaries.yml signer. Same-repository ./ calls
bind the caller commit; inline zizmor notes explain why current actionlint cannot
yet parse the newer $/ spelling. No broad audit suppression is configured.

## SBOM and release assets

The next authorized release has five archives, rust-mirror-VERSION.cdx.json, and
one SHA256SUMS covering the five archives AND SBOM in sorted order. All seven files
receive provenance; every archive additionally receives a CycloneDX SBOM attestation.
The generator is cargo-cyclonedx0.5.9, installed with --locked. It reads a clean
frozen source copy, keeps the lock byte-identical and verifies every registry
component checksum against it. SOURCE_DATE_EPOCH fixes time metadata; temporary
local package IDs are rewritten consistently to the exact source commit, preserving
the dependency graph; local file-download qualifiers are removed only from purls. Source commit and lock SHA-256 are recorded in the SBOM.

This is an all-target/all-feature Cargo dependency inventory, including build-time
dependencies. It is a superset for any individual target; it does not inventory OS
runtime libraries, SDKs, the Rust compiler or CI tools. An attestation binds this
scope to archive bytes, not a claim of complete binary composition or license review.

v0.5.1 retains four archives/one manifest; v0.5.2 retains five archives/one manifest.
Never attach an SBOM, second manifest or replacement asset to those immutable
releases under this task. A failed partial future upload needs an owner recovery
decision; no deletion/clobber repair is automated.

## Reproducibility measurements

[The measurement workflow](../.github/workflows/reproducible-builds.yml) builds each
of the five targets twice on independent fresh hosted runners without target
caches, using the same frozen source and pinned toolchain. It compares real binary
and archive SHA-256s and reports SBOM hashes. Its JSON/Markdown artifacts and PR
comment record the measured results; green measurement CI alone does not assert
that the bytes match.

Windows release builds scope /Brepro to the linker alongside static CRT to reduce
nondeterministic PE/PDB metadata; ordinary CI/development flags are unchanged.
Actual identity is still measured, not inferred from the flag.

Differences stay differences: the comparator identifies equal extracted contents
with different archive representation, and detects PE timestamp or Mach-O UUID/
signature-only changes for diagnosis. It never normalizes published bytes or treats
normalized matches as bit identity. Other differences are explicitly unresolved
and require review. Repeated-build success is bounded to those two environments,
not a universal reproducibility claim or proof on old native kernels.

## Optional Trusted Publishing

The existing coordinator/owner-machine process remains available. The new
[trusted-publishing.yml](../.github/workflows/trusted-publishing.yml) handles future v* tags.
A read-only job requires main ancestry, exact manifest/tag identity, one expected
workspace crate and verified packaging. If the manual process already published
that exact source, it verifies API/index/archive checksum and payload and skips
OIDC/publish. Existing versions with mismatched content or dirty VCS refuse.

For a missing version, the crates-io environment requires the owner's approval and
permits only v* tags. After approval the job rechecks registry state, exchanges
GitHub OIDC for a short-lived token using pinned rust-lang/crates-io-auth-action,
publishes rust-mirror with --locked and verifies the official checksum/clean-tag
payload. Authentication action cleanup revokes the token. No long-lived Cargo
secret, implicit fallback token or token in command arguments is introduced.
The one-crate workspace order is rust-mirror only; expanding it needs review.

The owner must register exactly this publisher in crates.io settings:

| Field | Value |
| --- | --- |
| Crate | rust-mirror |
| Repository owner | rustcommons |
| Repository name | rust-mirror |
| Workflow filename | trusted-publishing.yml |
| Environment | crates-io |

Configuration and an owner-approved real publication remain future validation;
this security PR performs no publication/token exchange. Do not create a test v*
tag, weaken protected tags or remove owner review to exercise it. For an optional
future tag-first Trusted Publishing release, approve promptly and create the exact
GitHub Release notes after registry confirmation; binary automation waits ten
minutes for that release and otherwise stops without uploading. A later approved
live dispatch at the exact immutable tag can retry when no assets exist. The manual publish-then-tag sequence
continues to work unchanged.
