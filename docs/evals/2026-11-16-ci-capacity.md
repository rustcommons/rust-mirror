# CI capacity amendment — 2026-11-16

The owner amended the baseline on November15 after the first Dependabot wave
blocked releases. Cargo/Actions version checks become monthly, two open PRs per
ecosystem, preserving minor/patch groups and the seven-day cooldown. Security
updates remain independent of those version-update limits/schedule.

[Routing cases](ci-capacity-20261116.json) use the ten actual open Dependabot PR
authors observed at audit time, with both bot and maintainer actors: all twenty
cases exclude heavy jobs while retaining one Ubuntu core job, cargo-deny, zizmor,
lf-only and managed CodeQL. Four further cases retain human-PR/main/manual/release
qualification and the normal two-OS core matrix. These are evaluated source
conditions grounded in live PR metadata, not claims that an unmerged bot workflow
has run on GitHub. The first merged-policy bot run remains future hosted evidence.

All13 repository PR-entry workflows have unique workflow/event/PR concurrency
groups. Cancellation is enabled for PR events, disabled for non-PR runs. Job guards
precede matrix expansion; callable cores inherit caller cancellation rather than
sharing a group across parallel repeat builds. Existing production build/sign and
serialized no-overwrite uploader job definitions are unchanged. Required lf-only
and core jobs have no author gate. Default CodeQL remains provider-managed;
its generated workflow concurrency is not controlled by repository YAML.

Actionlint1.7.12, zizmor1.32.0, the100-reference action-pin check, YAML parsing,
24 routing cases, links/rendering and the clean diff passed locally. No runtime,
manifest/lock, packaged README/changelog, source/test, license, shim, provider setting,
open bot branch or published release is changed by this amendment. Human PR CI
supplies hosted validation; actual queue-size reductions are not claimed in advance.

After merge, refresh bot branches and trigger fresh runs. Re-running old cancelled
runs keeps the original SHA/ref and may use the old heavy workflow. Full manual
qualification remains available before approving a migration; no bot PR was merged
or closed during this work.

Primary semantics: [Dependabot options](https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-options-reference),
[job condition before matrix](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idif),
[conditional concurrency](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency),
and [re-run source identity](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs).
