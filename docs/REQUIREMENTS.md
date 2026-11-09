# Requirements traceability

## Scope and sources

This index covers confirmed placeholder scope and documentation maintenance constraints
from the original audit, plus the owner's 2026-11-09 naming and product-direction
decision. `rust-mirror` is the planned core/CLI, with `rust-mirror-gui` a future GUI.
Detailed behavior and GUI technology remain undecided. A Windows-local prototype
was located and preserved; it has not been imported or qualified. The owner then
authorized the first pinned offline installation proof and Nexus Raw hosting work.
The new experimental implementation and bounded evidence are recorded separately
from the original placeholder audit below.

Sources are the [original README](https://github.com/rustcommons/rustup-offline-mirror/blob/190a9f326402febcd7a2d21cab059d6b2b871c86/README.md),
the [primary instructions at the audit baseline](https://github.com/rustcommons/rustup-offline-mirror/blob/190a9f326402febcd7a2d21cab059d6b2b871c86/AGENTS.md),
and the current [README](../README.md) and [AGENTS.md](../AGENTS.md).
The owner requested this placeholder documentation audit, the `ROM-REQ` prefix,
stable identifiers, and the shared traceability section. Those maintenance
constraints are recorded in [AGENTS.md](../AGENTS.md#documentation-and-requirements-traceability).
The [central conventions](https://github.com/rustcommons/.github/blob/main/docs/DOCUMENTATION.md)
are live. On 2026-11-01, the shared GitHub URL returned HTTP 200; the GitHub
Contents API confirmed the same file on `main` and at central revision
[`c8a0b9c01aae5657dd0ef649bd37b18d81919ce5`](https://github.com/rustcommons/.github/blob/c8a0b9c01aae5657dd0ef649bd37b18d81919ce5/docs/DOCUMENTATION.md),
with blob SHA `1b659ff536d43bcae5b09626069e89488058ba6f`. The initial audit
observed a 404 before the central merge; that historical observation is now resolved.

The shared [release documentation convention](https://github.com/rustcommons/.github/blob/a11ce9e8ab27848d5cf78cd0462a4d7182636c9a/docs/RELEASING.md)
is the source for the release instructions adopted on 2026-11-02. This placeholder
records release verification as Not applicable until its first authorized release.

“Verified” below means only the named source inspection, not runtime verification.
Other allowed statuses are “Implemented—partial verification”, “Planned”, “Deferred”,
and “Not applicable”. Future ideas are not approved requirements.

## Requirements

| ID | Requirement and source | Implementation / documentation | Verification evidence | Status and gaps |
| --- | --- | --- | --- | --- |
| ROM-REQ-001 | Preserve the original placeholder audit boundary and describe subsequent implementation only within newly authorized and verified scope. Source: baseline AGENTS.md, Project scope, and owner audit scope. | [README current state](../README.md#current-state); [AGENTS project scope](../AGENTS.md#project-scope). | Baseline tracked-file inspection at `190a9f326402febcd7a2d21cab059d6b2b871c86`: README, AGENTS, import shim, license, and ignore rules only. The current README distinguishes the new pinned-toolchain milestone from planned functionality. | Verified; source inspection only. Original audit boundary; planned product direction is now recorded in ROM-REQ-008/009. Local prototype remains unqualified. |
| ROM-REQ-002 | Require owner authorization for repository naming changes; do not invent capabilities or compatibility guarantees. Source: baseline AGENTS.md, Project scope, and owner audit scope. | [README](../README.md); [AGENTS project scope](../AGENTS.md#project-scope). | README and AGENTS title/scope inspection: original name retained at audit time. On 2026-11-09 the owner selected rust-mirror and authorized the rename; README and AGENTS record that decision. No compatibility guarantee asserted. | Verified; documentation inspection only. Naming decision completed; detailed implementation remains planned. |
| ROM-REQ-003 | Keep AGENTS.md primary and CLAUDE.md as its existing import shim; preserve common working and Git conventions. Source: baseline AGENTS.md and CLAUDE.md; owner audit instructions. | [AGENTS.md](../AGENTS.md); [CLAUDE.md](../CLAUDE.md). | Compare to baseline: CLAUDE.md unchanged; Working conventions and Git conventions preserved. | Verified; instruction-file inspection only. |
| ROM-REQ-004 | Maintain a linked trace index with stable repository-scoped IDs, bounded evidence, and explicit gaps; update affected documentation together. Source: owner audit instructions and AGENTS.md, Documentation and requirements traceability. | This index; [README documentation](../README.md#documentation); [shared instructions](../AGENTS.md#documentation-and-requirements-traceability). | Source inspection: required table columns and headings present; README and AGENTS link this index and central conventions; IDs are unique. Shared central link returned HTTP 200 on 2026-11-01; Contents API confirmed the file at the central revision named above. | Verified; documentation structure and shared-link availability only. |
| ROM-REQ-005 | Validate documentation using source/command claims, links, rendering, and diff checks; define actual setup/validation commands if implementation is added. Source: baseline AGENTS.md, Repository layout and validation; shared instructions. | [AGENTS validation](../AGENTS.md#repository-layout-and-validation); [maintenance](#maintenance). | Source inspection confirms documentation-only checks are specified and no build/setup commands are invented. | Verified; maintenance instructions only. Original instruction inspection is not build evidence; current commands and bounded implementation evidence are in AGENTS and ROM-REQ-011-014. |
| ROM-REQ-006 | Keep the original placeholder audit separate from later runtime implementation and verification. Source: owner audit scope and baseline tracked-file inventory. | [README current state](../README.md#current-state). | No runtime verification performed; no implementation, binaries, manifest, test suite, or build workflow in the audited repository. | Not applicable to the historical placeholder audit; current runtime evidence is separately bounded in ROM-REQ-014. |
| ROM-REQ-007 | Adopt the exact shared release-documentation instructions; do not invent versions, features, changelog history or GitHub Releases for this placeholder. Source: owner request and central release convention at `a11ce9e8ab27848d5cf78cd0462a4d7182636c9a`, Adoption and instructions. | [AGENTS release documentation](../AGENTS.md#release-documentation). | On 2026-11-02, exact section comparison with the supplied shared text and source inspection of the central convention confirmed instruction adoption; pre-change repository revision `cfdbcb91a220ed7de6655cc22bd89354d05b707f` remains a documentation-only placeholder. | Verified; instruction adoption only. Release verification is Not applicable until the first authorized release; no release, implementation or runtime verification is claimed. |
| ROM-REQ-008 | Name the core tool/CLI rust-mirror and preserve existing work when renaming the repository. Source: owner decision, 2026-11-09. | [README](../README.md); [AGENTS project scope](../AGENTS.md#project-scope). | GitHub repository renamed with the same repository identity and Git history; earlier local uncommitted prototype left untouched. | Verified; naming and preservation only. No published crate or qualified implementation. |
| ROM-REQ-009 | Plan rust-mirror-gui as a future graphical interface for rust-mirror workflows. Source: owner decision, 2026-11-09. | [README planned products](../README.md#planned-products). | Documentation records the selected name and planned relationship. | Planned; GUI framework, implementation, distribution and validation remain undefined. |
| ROM-REQ-010 | Assess and preserve the recovered prototype before reuse. Source: owner request, 2026-11-09. | [Assessment](evals/2026-11-09-prototype-assessment.md); [source inventory](evals/2026-11-09-prototype-inventory.json). | Separate-copy Windows test run: 8 original integration passes; 7 defects reproduced; all 21 original file hashes and original Git status unchanged. | Verified; bounded prototype assessment only. No implementation import, release or end-to-end mirror qualification. |
| ROM-REQ-011 | Prepare one pinned minimal-profile toolchain snapshot through reusable core logic. Source: owner first offline-install milestone, 2026-11-09. | [Core](../src/lib.rs); [CLI](../src/main.rs); [guide](NEXUS-RAW.md). | Rust 1.90.0 / Rustup 1.28.2 / x86_64 Linux proof; upstream metadata retained and only selected host archives downloaded. | Implemented?partial verification; other release/client/host/component combinations remain unqualified. |
| ROM-REQ-012 | Enforce path/byte/hash checks and refuse overwriting an existing output. Source: authorized proof and assessed prototype defects. | [Core](../src/lib.rs); [snapshot tests](../tests/snapshot.rs). | Focused path, schema, compression/hash, foreign-origin/target, existing-output and failed-publication cases; real upstream hashes retained in the inventory. | Implemented?partial verification; private-parent staging only, no general cache/concurrency or independent OpenPGP guarantee. |
| ROM-REQ-013 | Support a static Nexus Raw-style layout and verified HTTP PUT publication. Source: owner Nexus-OSS Raw hosting requirement, 2026-11-09. | [Publisher](../scripts/publish_raw.py); [Nexus guide](NEXUS-RAW.md). | Local HTTP PUT/HEAD/GET protocol fixture and fresh repository-prefix install; publisher verifies local and remote bytes. | Implemented?partial verification; real Nexus product/TLS/auth/policy test pending. No real endpoint writes performed. |
| ROM-REQ-014 | Prove fresh offline bootstrap/install and dependency-free compilation with negative controls. Source: owner proof request, 2026-11-09. | [Driver](../scripts/prove_offline.py); [evidence](evals/2026-11-09-offline-toolchain.md). | Isolated namespace, loopback-only/no route, fresh homes, successful install/build/run; missing and corrupt archive controls fail as expected. | Verified; exact recorded Linux/Rustup/Rust case only. No crates.io, native-prerequisite provisioning, other platforms or general compatibility claim. |

## Documentation map

| Document | Role |
| --- | --- |
| [README.md](../README.md) | Public purpose, current state, prerequisites/setup availability, and guide links. |
| [AGENTS.md](../AGENTS.md) | Primary scope, validation, traceability, working, and Git instructions. |
| [CLAUDE.md](../CLAUDE.md) | Unchanged import shim for AGENTS.md. |
| [docs/REQUIREMENTS.md](REQUIREMENTS.md) | Confirmed constraints, evidence boundaries, and gaps. |
| [Prototype assessment](evals/2026-11-09-prototype-assessment.md) | Dated source assessment, reproduced gaps and proposed milestones; not release qualification. |
| [Nexus hosting guide](NEXUS-RAW.md) | Current preparation, transfer, publication and client setup with limits. |
| [Offline proof](evals/2026-11-09-offline-toolchain.md) | Dated exact offline-install evidence, negative controls and gaps. |
| [CHANGELOG.md](../CHANGELOG.md) | Unreleased implementation changes; no fabricated release history. |
| [LICENSE](../LICENSE) | Existing license, unchanged. |
| [Central conventions](https://github.com/rustcommons/.github/blob/main/docs/DOCUMENTATION.md) | Live shared documentation policy; link verified 2026-11-01 as recorded above. |
| [Central release convention](https://github.com/rustcommons/.github/blob/main/docs/RELEASING.md) | Shared release-documentation instructions; release verification is Not applicable for this placeholder. |

No implementation guides or validation reports existed in the original audit
baseline. Current guides/evidence above document the separately authorized milestone;
no release records are fabricated.

## Maintenance

- Preserve existing IDs; never renumber or reuse them. Allocate new `ROM-REQ` numeric
  suffixes only for newly confirmed requirements, with their sources.
- Update this index, README, and primary instructions together when confirmed scope,
  documentation, prerequisites, or validation changes. Record missing evidence explicitly.
- Check relative links and anchors, Markdown rendering, source and command claims,
  instruction-file consistency, and `git diff --check` for documentation edits.
- Keep dated evidence tied to an exact revision; do not rewrite historical outcomes.
  Source inspection cannot establish runtime behavior or change an immutable package.
- For the original placeholder audit, no builds were applicable. The current
  experimental implementation uses the commands in AGENTS; do not treat bounded
  proof coverage as a published-release or all-platform guarantee.
- On 2026-11-01, GitHub merge metadata and Git ancestry inspection confirmed
  [PR #1](https://github.com/rustcommons/rustup-offline-mirror/pull/1) merged at
  [`d192d575217b33731c6a4f78fe946ede353a29f2`](https://github.com/rustcommons/rustup-offline-mirror/commit/d192d575217b33731c6a4f78fe946ede353a29f2),
  preserving reviewed head `6706689c997ab1f442af857771aa98eb16c08a54`.
- Shared-link checks after the central conventions merge were completed on
  2026-11-01 as recorded above; recheck affected links when documentation targets change.
