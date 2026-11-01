# Requirements traceability

## Scope and sources

This index covers confirmed placeholder scope and documentation maintenance constraints
only. It does not approve functional requirements. Naming and product scope remain
undecided. Windows-local prototypes and notes were not available or reviewed.

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

“Verified” below means only the named source inspection, not runtime verification.
Other allowed statuses are “Implemented—partial verification”, “Planned”, “Deferred”,
and “Not applicable”. Future ideas are not approved requirements.

## Requirements

| ID | Requirement and source | Implementation / documentation | Verification evidence | Status and gaps |
| --- | --- | --- | --- | --- |
| ROM-REQ-001 | Describe the repository as a placeholder; do not present the original mirror idea as shipped behavior or an approved functional requirement. Source: baseline AGENTS.md, Project scope, and owner audit scope. | [README current state](../README.md#current-state); [AGENTS project scope](../AGENTS.md#project-scope). | Baseline tracked-file inspection at `190a9f326402febcd7a2d21cab059d6b2b871c86`: README, AGENTS, import shim, license, and ignore rules only. Current README explicitly bounds the idea. | Verified; source inspection only. Product scope remains undecided; local prototypes and notes are unreviewed. |
| ROM-REQ-002 | Preserve the repository name while naming and scope are under review; do not invent capabilities or compatibility guarantees. Source: baseline AGENTS.md, Project scope, and owner audit scope. | [README](../README.md); [AGENTS project scope](../AGENTS.md#project-scope). | README and AGENTS title/scope inspection: existing name retained, provisional scope stated, no compatibility guarantee asserted. | Verified; documentation inspection only. Naming decision remains open. |
| ROM-REQ-003 | Keep AGENTS.md primary and CLAUDE.md as its existing import shim; preserve common working and Git conventions. Source: baseline AGENTS.md and CLAUDE.md; owner audit instructions. | [AGENTS.md](../AGENTS.md); [CLAUDE.md](../CLAUDE.md). | Compare to baseline: CLAUDE.md unchanged; Working conventions and Git conventions preserved. | Verified; instruction-file inspection only. |
| ROM-REQ-004 | Maintain a linked trace index with stable repository-scoped IDs, bounded evidence, and explicit gaps; update affected documentation together. Source: owner audit instructions and AGENTS.md, Documentation and requirements traceability. | This index; [README documentation](../README.md#documentation); [shared instructions](../AGENTS.md#documentation-and-requirements-traceability). | Source inspection: required table columns and headings present; README and AGENTS link this index and central conventions; IDs are unique. Shared central link returned HTTP 200 on 2026-11-01; Contents API confirmed the file at the central revision named above. | Verified; documentation structure and shared-link availability only. |
| ROM-REQ-005 | Validate documentation using source/command claims, links, rendering, and diff checks; define actual setup/validation commands if implementation is added. Source: baseline AGENTS.md, Repository layout and validation; shared instructions. | [AGENTS validation](../AGENTS.md#repository-layout-and-validation); [maintenance](#maintenance). | Source inspection confirms documentation-only checks are specified and no build/setup commands are invented. | Verified; maintenance instructions only. This is not build or test evidence. |
| ROM-REQ-006 | Runtime implementation and verification are outside the confirmed placeholder audit scope. Source: owner audit scope and baseline tracked-file inventory. | [README current state](../README.md#current-state). | No runtime verification performed; no implementation, binaries, manifest, test suite, or build workflow in the audited repository. | Not applicable; no functional requirements, runtime results, or compatibility claims established. |

## Documentation map

| Document | Role |
| --- | --- |
| [README.md](../README.md) | Public purpose, current state, prerequisites/setup availability, and guide links. |
| [AGENTS.md](../AGENTS.md) | Primary scope, validation, traceability, working, and Git instructions. |
| [CLAUDE.md](../CLAUDE.md) | Unchanged import shim for AGENTS.md. |
| [docs/REQUIREMENTS.md](REQUIREMENTS.md) | Confirmed constraints, evidence boundaries, and gaps. |
| [LICENSE](../LICENSE) | Existing license, unchanged. |
| [Central conventions](https://github.com/rustcommons/.github/blob/main/docs/DOCUMENTATION.md) | Live shared documentation policy; link verified 2026-11-01 as recorded above. |

No implementation guides, roadmap, release records, or validation reports exist in
the audit baseline. The original idea is not promoted into an approved roadmap.

## Maintenance

- Preserve existing IDs; never renumber or reuse them. Allocate new `ROM-REQ` numeric
  suffixes only for newly confirmed requirements, with their sources.
- Update this index, README, and primary instructions together when confirmed scope,
  documentation, prerequisites, or validation changes. Record missing evidence explicitly.
- Check relative links and anchors, Markdown rendering, source and command claims,
  instruction-file consistency, and `git diff --check` for documentation edits.
- Keep dated evidence tied to an exact revision; do not rewrite historical outcomes.
  Source inspection cannot establish runtime behavior or change an immutable package.
- Do not run builds or install dependencies for this placeholder. Add actual setup and
  validation guidance only when implementation exists and its scope is confirmed.
- Keep this audit PR unmerged pending independent coordinator approval. Shared-link
  checks after the central conventions merge were completed on 2026-11-01 as recorded
  above; recheck affected links when documentation targets change.
