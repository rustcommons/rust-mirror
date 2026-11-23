# Tracked crate regression corpus

The owner authorized this suite on 2026-11-23. It turns the dated qualification
corpora into reviewed regression inputs. Resolution and planning remain native;
Cargo is a CI oracle only and is never invoked by the product.

## Coverage and execution

The versioned [corpus README](../tests/corpus/README.md) describes each input and
its limits. Definitions carry schema 1, a creation date and source. Top roots use
exact reviewed versions, explicit features and default-feature flags. The current
ranking has 100 roots; tricky cases add named feature/target/links/yank/collision
profiles, and four pinned real project locks retain their original bytes.

Build the checkout's CLI, then run metadata-only planning locally:

```sh
cargo build --locked --bin rust-mirror --jobs 2
python3 scripts/corpus.py run --binary target/debug/rust-mirror --output /tmp/corpus-run
```

Use a new output directory. `--fast` keeps the two complete top-100 native plans,
all synthetic cases, selected live cases and one lock project; CI samples eight
Cargo root profiles per top-100 plan. Full weekly/manual CI checks all roots and
all lock projects. CI adds `--oracle`, which refuses outside GitHub Actions.
Local reports qualify planning only; they do not substitute for CI conformance.
Python uses only its standard library. The loopback TLS fixture uses the existing
OpenSSL certificate helper without installing roots into the machine trust store.

Planning goes through the actual CLI and its fixed-origin, certificate-verified
transport. A bounded, owned CONNECT fixture supplies either the committed frozen
observations or a coherent live sparse-index snapshot. It refuses all archive GETs,
HEADs and non-index CONNECTs. Registry concurrency is one; at most four local
request handlers run. Metadata is bounded to 32 MiB per index and 512 MiB per run.
The existing product User-Agent/contact is used for upstream requests. An optional
private `--cache DIRECTORY` preserves ETags and Last-Modified values across runs;
conditional HTTP 304s reuse original bytes. Within one run every observer, including
Cargo, receives the same bytes. CI hosts are ephemeral and start with a fresh cache.
No repeated ranking API crawl or archive download is hidden in ordinary runs.

## Reports and metrics

`report.json` contains exact source/binary/corpus identities, per-case and per-root
resolved versions, union, original record identities/reasons, index collisions,
metadata warnings, typed refusals and timings. CI records independent native-root
and full/filtered Cargo timings for sampled/all multi-root profiles. A local aggregate
plan has only aggregate timing; it does not pretend to measure each root separately.
`summary.md` is the short verdict. JSON, Cargo locks/logs and baseline proposals are
retained as workflow artifacts, excluding private keys, Cargo homes and archives.

Cargo independently generates lockfiles against full and proposed filtered sparse
indexes, and must resolve within native per-root coverage. For imported real locks,
Cargo checks exact registry aliases with the original lock as preferences using
`cargo update --workspace`; every registry identity/checksum must equal the native
pin union in both indexes. This is registry-projection conformance, not a project
build or a reconstruction of git/path/workspace dependencies and feature settings.
Native frozen-plan `show` must round-trip its saved data. The events adapter's
optional null fields are normalized to TOML's omitted-field representation; false,
zero, empty arrays and every non-null field remain significant.

Version overfetch is the native-only set divided by the Cargo set. `known_archive_bytes`
and `unknown_archives` explicitly distinguish known lengths from missing estimates.
Sparse index records do not provide archive lengths; this suite deliberately makes
no CDN HEAD/GET calls, so current byte estimates can be zero-known/all-unknown and
byte overfetch is null, never claimed as 0%. The historical 45,125,358-byte measurement
is separately labeled as evidence from the original qualification, not remeasured.

## Baseline and classification rules

Every case first replays the exact committed metadata with the current binary,
then plans against live metadata. Committed deterministic expected results exclude
wall time, observation timestamps and plan IDs, but include original index-line
hashes and native provenance. This frozen control is mandatory even when upstream
metadata changed concurrently.

- **Clean:** frozen replay, expected output and live inputs/output agree.
- **Our regression:** current output differs on frozen inputs, output changes with
  unchanged inputs, a named edge-case expectation breaks, or an unexplained refusal
  appears. Missing expected cases fail rather than teaching the runner new behavior.
- **Upstream (report, not fail):** frozen control passes, Cargo conformance passes,
  and an explicit input diff demonstrates only new versions/indexes, yank changes,
  nonidentity backfills or formatting changes. A new refusal is excused only when
  a declared root itself demonstrably became yanked. An unrelated new version
  cannot excuse it. Mutated/removed existing checksum/dependency/feature/links/schema
  identities fail conservatively; they are never silently accepted as a backfill.
- **Conformance mismatch:** Cargo/full/filtered coverage, refusal or locked checksum
  results disagree, regardless of concurrent upstream changes. Always fail.
- **Infrastructure failure:** a timeout, missing output or transport/format error
  prevents qualification. Fail and report it; never propose it as an upstream update.

The [classification tests](../tests/test_corpus.py) exercise these rules, including
concurrent upstream changes that must not mask a frozen-input regression.
The frozen 2026-11-13 control must still reproduce 100/100 roots and the exact
186-version union; a changed total fails. It is distinct from the current ranking.

## Refresh and reviewed baseline proposals

The frozen [2026-11-13 fixture](../tests/fixtures/crates-top100-20261113.json) is
immutable. Refresh only the current ranking explicitly:

```sh
python3 scripts/corpus.py refresh --report /tmp/root-refresh.json
# Review added_roots, removed_roots, changed_profiles and the .proposal.json first.
python3 scripts/corpus.py refresh --write --report /tmp/root-refresh-reviewed.json
```

A refresh performs one bounded official crates.io API request. Existing declared
features/default choices are preserved for retained names; new roots receive
explicit defaults enabled and an empty extra-feature list. Every refresh is a PR
showing added/removed roots and profile/version changes, even when both root-name
lists are empty. The initial 2026-11-23 refresh retains all 100 names, updates 16
root versions, and leaves the frozen fixture untouched.

When live inputs change, the artifact contains `proposed-baseline`, `baseline.patch`
and `baseline-update.md`, a proposed baseline-update PR description. Nothing is
pushed, committed, opened or merged automatically. Review the full JSON metadata
deltas and proposed files. Require a full dispatch before copying proposals into
`tests/corpus`, then run another full dispatch on the baseline-update head. A PR
fast report is an incomplete proposal and never a full release qualification.
Local `--record` can bootstrap deterministic expectations into the output directory;
it never edits committed baselines and is not conformance proof. A failed report
must be investigated before any proposed baseline is accepted. The Repository
Review Agent reviews this suite and later intentional behavior changes before merge.

## Weekly issue and release gate

[corpus.yml](../.github/workflows/corpus.yml) runs weekly, on dispatch and on relevant
PRs; Dependabot PRs skip it. Only the weekly tracking job has `issues: write`.
It opens or updates one issue titled **Crate corpus report** when something changed
or failed, reopens that same issue after a later change, and closes it when clean.
Duplicate existing tracking titles cause a refusal rather than another issue.
Manual dispatch and PR jobs never write issues. There is no automatic baseline PR
or merge; the tracking issue points to report/proposal artifacts.

Before a release, the latest weekly run must have succeeded with every difference
clean or classified upstream, or a full dispatch on the exact release commit must
pass. The tag release gate enforces this through:

```sh
python3 scripts/corpus.py release-check --repository rustcommons/rust-mirror --sha FULL_COMMIT_SHA
```

A failed/in-progress latest weekly cannot be replaced by an older successful weekly;
only the exact-head dispatch alternative can clear it. See the
[release checklist](RELEASING.md#before-merge-and-publication). No new dependency,
version bump, software release or production registry/Nexus operation is part of
maintaining this suite.

## Initial qualification, 2026-11-23

[Fast PR run 45112299093](https://github.com/rustcommons/rust-mirror/actions/runs/45112299093)
passed all 16 cases. [Full branch dispatch 45112301249](https://github.com/rustcommons/rust-mirror/actions/runs/45112301249)
on implementation commit `9c8277b23492937a4928e47068f2447abb0eaf50` passed all
21 cases, 219 Cargo profiles and three native guards. Both reports were clean,
with no upstream differences or archive requests. Each top-100 corpus reproduced
100/100 roots and matching native/Cargo 186-version unions. The frozen union also
matched every version identity in the original retained qualification artifact.
All four lock projections preserved their 55/153/51/125 registry identities and
checksums. Reports are attached to [PR #84](https://github.com/rustcommons/rust-mirror/pull/84);
artifact retention is 30 days, while inputs and expected results remain committed.
This is bounded metadata-resolution evidence, not archive integrity, compilation,
project build, arbitrary feature completeness or perpetual upstream compatibility.
