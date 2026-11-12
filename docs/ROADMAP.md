# Roadmap

Updated 2026-11-12. This is future work, not a release promise or a description of
implemented functionality. Current behavior is in the [user guide](USER-GUIDE.md),
[network contract](NETWORK.md), [retention contract](RETENTION.md) and
[requirements index](REQUIREMENTS.md). No new version is assigned here.

## Curated crate mirroring: proposed phases

The owner authorized a **design only** review of [CRATES-DESIGN.md](CRATES-DESIGN.md).
Implementation, dependencies and a release need their own approval. Estimates below
are rough engineering effort for one maintainer, excluding review/approval delays,
infrastructure access and unexpected upstream changes. They are not delivery dates.

| Phase | Smallest useful outcome and exit gate | Rough effort |
| --- | --- | --- |
| 0: resolver feasibility | Prove Cargo-syntax ingestion, isolated metadata-only Cargo resolution and filtered sparse-index witness replay, with no archive downloads. Decide dependencies, Cargo baseline and coverage policy before building the rest. | 3–5 working days; stop/review if the metadata-only assumption fails. |
| 1: MVP | Organization catalog with aliases, several ranges, latest N and exact pins; declared features and all-platform normal/build closure, dev excluded; bounded preview/reasons/size estimates; frozen fetch/resume; retained versions; portable immutable static sparse output; controlled library/events; new-prefix publication and crate-aware whole-generation protection. Fresh Windows/Unix Cargo clients and failure/resume/closure qualification required. | 10–15 additional working days after phase 0 and approvals. |
| 2: broader coverage and GUI | Optional broader feature scenarios and any proven target-size optimization, richer why-included review, GUI catalog/preview/sync/publish integration, additional Cargo/resolver/MSRV baselines. Preserve immutable output and previewed pruning. | 5–10 additional working days; GUI work estimated separately by its maintainer. |
| 3: additional input and registry modes | User Cargo.lock pin import, alternate/private registries, authenticated readers or native Nexus Cargo support, each with a separate design and qualification gate. | Not estimated until scope, identity/authentication and owner priorities are agreed. |

The MVP is useful across projects because the catalog holds independently resolved
root versions and retains older mirrored data. It does not require importing any
project's lockfile. A frozen plan's internally generated resolver lock is not user
Cargo.lock import. A failed compatibility/resolution gate blocks release; do not
replace Cargo solving with an unchecked greedy traversal to meet an estimate.

## Deferred crate work

| Item | Boundary and condition for reconsideration |
| --- | --- |
| Cargo.lock import | Explicitly future work. Merge exact registry/name/version/checksum pins as another provenance-bearing source; keep manual rules and independent protection. Define yanked historical-pin handling and feature/target policy before implementation. |
| All optional dependencies and arbitrary project combinations | MVP covers declared feature profiles and Cargo's lock closure, not every feature combination or one organization-wide solvable workspace. Add bounded scenario policies and Cargo client evidence before broader claims. |
| Target-minimized archives | Keep conservative all-platform lock closure first. Prove Cargo metadata/lock behavior and completeness with a filtered index before omitting archives for inactive targets. |
| New yanked pins | Existing verified retained versions can survive. A new yanked exact pin needs a safe resolver/import policy; do not unyank or rewrite upstream metadata. |
| Alternate/private registries and authenticated Cargo readers | Require source identities, credential-provider policy, origin allowlists and client qualification; no silent mapping to crates.io or embedded secrets. |
| Git/path/workspace inheritance import | Refuse clearly in the registry catalog. No traversal, Git checkout, source substitution or full-project manifest import is approved. |
| Native Nexus Cargo publishing | Separate from Raw/static publication; verify exact source identity, population, auth, index/retention and disconnected behavior. Existing Raw evidence does not cover it. |
| Bulk registry crawl/database dumps | Curated sparse access only. Most/all-crates mirroring requires a different capacity/policy design, not increasing this planner's caps. |
| In-generation crate compaction and stable activation routing | Successor generations and explicit whole-generation retirement first; prove cache/concurrent-client safety before mutable aliases or record-level deletion. |
| License/security approval automation | Closure review and upstream checksum integrity do not establish redistribution permission, vulnerability absence or build safety. Organization policy/tools need separate authorization. |

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
