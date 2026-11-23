# Roadmap

Updated 2026-11-23 (OpenPGP dependency gate); crate items reflect owner decisions dated 2026-11-12. This is future work, not a release promise or a description of
implemented functionality. Current behavior is in the [user guide](USER-GUIDE.md),
[network contract](NETWORK.md), [retention contract](RETENTION.md) and
[requirements index](REQUIREMENTS.md). The owner-approved lock-import iteration shipped in 0.5.0 on 2026-11-13;
[publication evidence](evals/2026-11-13-publication-0.5.0.md) identifies its bounded scope.

## OpenPGP manifest verification: blocked on upstream dependencies

OpenPGP manifest verification: implemented on branch
[feat/openpgp-manifests](https://github.com/rustcommons/rust-mirror/tree/feat/openpgp-manifests);
blocked until rPGP depends on rsa >= 0.10 and crypto-bigint >= 0.7.6; resume by
updating pgp, confirming cargo-deny is clean with no exceptions, then opening the
feature PR.

The branch currently contains a verification prototype and dependency assessment,
not completed product integration. Plan/inventory, opt-out, migration, negative
and fuzz tests, documentation and full qualification remain unfinished. Main's
existing toolchain behavior is unchanged. Owner decision, 2026-11-22, option B:
NO advisory exceptions; the `pgp` dependency and pinned-key approvals still stand.
Wait for a released compatible graph, then recheck MSRV 1.90, licenses and the
full resolved graph as well as both named advisories. Never select a yanked fix.

Existing upstream tracker: [rPGP PR #573, Adopt RustCrypto new releases](https://github.com/rpgp/rpgp/pull/573)
(open when checked 2026-11-23; its inspected diff proposes rsa 0.10 and
crypto-bigint 0.7.11). [Issue #257](https://github.com/rpgp/rpgp/issues/257) tracks
the RSA advisory. No upstream issue or PR was opened. See the
[dependency/key assessment](evals/2026-11-23-openpgp-dependency-assessment.md) and
[owner authorization](../AGENTS.md#openpgp-manifest-verification-and-070-authorization).
The preserved branch has no feature PR; 0.7.0 preparation remains separate after
feature review and merge.

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

- **Phase 2 done (2026-11-21).** The owner approved folding the binaries in.
  trusted-publishing.yml now builds the crate and all five binaries through the
  read-only core with every qualification gate, writes one SHA256SUMS and one SBOM,
  signs it inside the same isolated reusable workflow that built it (build-binaries.yml,
  Build / Checksums and attestations) before the owner's approval, creates the release
  as a draft, uploads all nine assets and then publishes it and verifies the crate
  plus the public Windows/GNU/musl downloads. release-binaries.yml is retired, so the
  late-approval collector wait is gone. The PR dry run built every target and the
  crate with publish=false.

Deferred, not lost:

- **First tag run.** Build / Checksums and attestations, `publish`, `github-release`
  and `verify` run only on a tag; their first real use is the next authorized release. Record the signer
  identity and bundle subjects, the store match, the nine-asset upload, re-run
  behaviour, docs.rs and the three public-download legs in that release's record.
- **rust-mirror-gui copies phase 2.** Its pipeline should follow this pilot after the
  first real rust-mirror tag run confirms it.
- **One SBOM for crate and binaries.** The SBOM is the locked all-target Cargo graph,
  identical for the crate and every archive, and is bound to each archive and the
  crate. Per-target SBOMs are not planned.
- **Release-PR audit overlap.** `build` repeats the package audit and packaged
  install at the tag. The manual release-PR checks stay until a release proves the
  pipeline; then the release PR can cite the dry run instead.
- **Reviving release-binaries.yml.** GitHub dispatches only workflows that exist on
  the default branch. If a release up to 0.6.0 ever needs exact-tag recovery, a
  reviewed PR must restore the file first; none is needed today.
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
