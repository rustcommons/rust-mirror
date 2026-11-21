# Roadmap

Updated 2026-11-21 (standard release pipeline); crate items reflect owner decisions dated 2026-11-12. This is future work, not a release promise or a description of
implemented functionality. Current behavior is in the [user guide](USER-GUIDE.md),
[network contract](NETWORK.md), [retention contract](RETENTION.md) and
[requirements index](REQUIREMENTS.md). The owner-approved lock-import iteration shipped in 0.5.0 on 2026-11-13;
[publication evidence](evals/2026-11-13-publication-0.5.0.md) identifies its bounded scope.

## Curated crate mirroring: proposed phases

The coordinator accepted Phase 0 and normal-merged PRs #30/#31. The owner approved
Phase 1 on 2026-11-13; its accepted implementation and bounded gates are
recorded in [MVP qualification](evals/2026-11-13-crates-mvp.md) and [CRATES.md](CRATES.md).
Later phases, including GUI work, require separate owner authorization.
Runtime external/downloaded Cargo and the Cargo library remain excluded. Existing
and approved dependencies only. The owner separately authorized the 0.4.0 release
on 2026-11-13; see [release notes](releases/v0.4.0.md).
Estimates below
are rough engineering effort for one maintainer, excluding review/approval delays,
infrastructure access and unexpected upstream changes. They are not delivery dates.

| Phase | Smallest useful outcome and exit gate | Rough effort |
| --- | --- | --- |
| 0: resolver feasibility | Implement native conservative closure, feature propagation and bounded consistency search; prove filtered sparse-index conformance with real Cargo used only in CI. Include representative popular-crate corpus, tricky fixtures, zero planning archives and measured version overfetch. No runtime Cargo or third-party solver. | 5–10 working days; revised upward for native consistency search. Stop/review on unsupported semantics or conformance failure. |
| 1: MVP | Organization catalog with aliases, several ranges, latest N and exact pins; approved declared features and all-platform normal/build closure, dev excluded; bounded preview/reasons/size estimates; frozen fetch/resume; retained versions; portable immutable static sparse output; controlled library/events; new-prefix publication and crate-aware whole-generation protection. Fresh Windows/Unix Cargo clients and failure/resume/closure qualification required. | 10–15 additional working days after phase 0 and approvals. |
| 2: broader coverage and GUI | Additional explicitly requested declared-feature scenarios and any proven target-size optimization, richer why-included review, GUI catalog/preview/sync/publish integration, additional Cargo/resolver/MSRV baselines. Preserve immutable output and previewed pruning. | 5–10 additional working days; GUI work estimated separately by its maintainer. |
| 3: additional input and registry modes | User Cargo.lock pin import, alternate/private registries, authenticated readers or native Nexus Cargo support, each with a separate design and qualification gate. | Not estimated until scope, identity/authentication and owner priorities are agreed. |

The MVP is useful across projects because the catalog holds independently resolved
root versions and retains older mirrored data. It does not require importing any
project's lockfile. A frozen plan's internally generated resolver lock is not user
Cargo.lock import. A failed consistency/conformance gate blocks release. Newest-per-edge alone is not
a proof: intersecting requirements can lose a needed candidate through prerelease
eligibility or per-version features. Add bounded native witness search/repair, not a
runtime Cargo fallback or an unchecked greedy success claim.

Phase 0 work and its bounded local results are tracked in the
[native resolution record](evals/2026-11-13-native-crates-phase0.md). That record names
the qualified source, Cargo conformance CI and measured counts; this does not
grant publication authority. Phase 1 implementation has separate owner approval
as recorded above.

## Completed iteration: lock import

On 2026-11-13 the owner approved the Cargo.lock input subset of Phase 3 for 0.5.0.
[Contract](LOCK-IMPORT.md) was reviewed/merged first; implementation and
[bounded evidence](evals/2026-11-13-lock-import.md) follow it. Named copied pin sources,
reimport/removal diffs, v1–v4 checksums, source-scoped yanks and served-index checks
must be self-contained and preserve manual rules. Real-project Linux/Windows Cargo
qualification and existing gates were required. The coordinator published from the
owner's machine after review/merge; new dependencies
and other Phase 3 registry modes or GUI work remain separately governed.

## Deferred crate work

| Item | Boundary and condition for reconsideration |
| --- | --- |
| Broader historical profile recomputation | The MVP preserves prior closures and replays their combined index. If a mix requires an additional unmirrored feature edge, it refuses; expanding/repairing that mix needs new native provenance and approval evidence. |
| All optional dependencies and arbitrary project combinations | Approved MVP covers declared feature profiles and native all-platform closure, not every feature combination or one organization-wide solvable workspace. Add bounded scenario policies and Cargo client evidence before broader claims. |
| Target-minimized archives | Keep conservative all-platform lock closure first. Prove Cargo metadata/lock behavior and completeness with a filtered index before omitting archives for inactive targets. |
| Manual new yanked pins | Continue refusing new manual selections. Planned lock-imported exact pins use the separate checksum/provenance gate; never unyank or rewrite upstream metadata. |
| Alternate/private registries and authenticated Cargo readers | Require source identities, credential-provider policy, origin allowlists and client qualification; no silent mapping to crates.io or embedded secrets. |
| Git/path/workspace inheritance import | Refuse clearly in the registry catalog. No traversal, Git checkout, source substitution or full-project manifest import is approved. |
| Native Nexus Cargo publishing | Separate from Raw/static publication; verify exact source identity, population, auth, index/retention and disconnected behavior. Existing Raw evidence does not cover it. |
| Bulk registry crawl/database dumps | Curated sparse access only. Most/all-crates mirroring requires a different capacity/policy design, not increasing this planner's caps. |
| In-generation crate compaction and stable activation routing | Successor generations and explicit whole-generation retirement first; prove cache/concurrent-client safety before mutable aliases or record-level deletion. |
| License/security approval automation | Closure review and upstream checksum integrity do not establish redistribution permission, vulnerability absence or build safety. Organization policy/tools need separate authorization. |

## Standard release pipeline

rust-mirror adopted the Rust Commons
[standard release pipeline](https://github.com/rustcommons/.github/blob/main/docs/TRUSTED-PUBLISHING.md)
on 2026-11-21, following the cargo-libc pilot (cargo-libc#43); see
[the release flow](RELEASING.md#standard-release-pipeline). The PR dry run exercised
`gate` and `build` with publish=false, and the gate passed locally against the
existing v0.6.0 tag. Deferred, not lost:

- **First tag run.** `attest`, `publish`, `github-release` and `verify` run only on a
  tag; their first real use is the next authorized release. Record attestation
  signing, bundle naming, `gh attestation verify --bundle`, the docs.rs wait and
  re-run uploads, and how release-binaries.yml attaches to the release it finds.
- **Phase 2: binaries in this pipeline** (planned, not approved). Move the five
  binary builds, the GNU userland gates and their attestation into `build`/`attest`,
  with one SHA256SUMS and one SBOM, keeping every qualification gate. Until then the
  crate SBOM (`.crates.cdx.json`) and the binaries' SBOM (`.cdx.json`) are separate
  files generated the same way from the same lock.
- **Binary collector wait.** release-binaries.yml still waits only 600 s for the
  GitHub Release, which now appears after owner approval. A later approval still
  needs the exact-tag dispatch, as for 0.6.0. Phase 2 removes the wait; changing it
  earlier needs approval because build-binaries*.yml is frozen for this adoption.
- **Gate CI query.** GitHub's `status=success` run filter omitted v0.6.0's
  successful CI run, so the gate filters `conclusion` itself. cargo-libc's gate
  and the standard should take the same fix.
- **Spec clarification.** As cargo-libc noted, `attest`, `github-release` and
  `verify` also list `gate` in `needs` to read its output.
- **Release-PR audit overlap.** `build` repeats the package audit and packaged
  install at the tag. The manual release-PR checks stay until a release proves the
  pipeline; then the release PR can cite the dry run instead.
- **Already-published versions.** 0.6.0 and earlier keep their hand-created
  releases and assets; none is re-tagged or re-released.

## Existing cross-cutting deferrals

These remain outside the crate-design task; they are not new implementation approval:

- Remote pruning: [the Nexus gap](RETENTION.md#granularity-and-nexus-gap) remains
  deferred. No real Nexus deletion is authorized.
- Exact config-aware resume compatibility inspection and whole private-workspace
  cleanup: [GUI retention limits](RETENTION.md#discover-generations-and-sync-runs),
  tracked by ROM-REQ-035. Protect paused runs and next-previous dependencies first.
- Scheduling, shared writable caching and in-generation toolchain compaction:
  current [sync](SYNC.md), [resume](RESUMING.md) and
  [multi-version](MULTI-VERSION.md) boundaries continue to apply.
- Independent signature verification and unqualified native platforms remain
  evidence gaps, not inferred guarantees from metadata or checksum validation.

Update affected requirements, protocol/API documentation and this roadmap together
when an item receives implementation approval. Record completed behavior and dated
qualification in their appropriate guides/evidence; preserve historical outcomes.
