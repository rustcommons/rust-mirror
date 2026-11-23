# Tracked crate corpora

Schema 1, created 2026-11-23. Definitions and expected results are reviewed source,
not automatically refreshed test data. See [CORPUS.md](../../docs/CORPUS.md).

- `top-crates.json`: 100 roots from the official downloads ranking, exact versions,
  explicit feature lists and default-feature choices. Covers curated default
  profiles, not every feature or project combination. Only `corpus.py refresh`
  queries the ranking API; regular runs use the committed roots and sparse index.
- `tricky.json`: named, reasoned synthetic and live profiles covering feature
  intersections, prereleases, links, yanks, renames/features2, target/build/dev edges,
  reqwest/default, tokio/full, windows-sys and ndk-sys build-metadata collisions.
  Its CI guards additionally exercise historical backfills, features2 identity and
  generation output preservation with the checkout's Rust tests. These are finite
  cases, not arbitrary Cargo completeness or production publication proofs.
- `locks.json` and `locks/*.lock`: original project locks at the commits and
  SHA-256 values from `tests/fixtures/crates-lock-corpus.json`. Covers registry pin
  and checksum preservation, including locked yanks. The CI Cargo oracle checks a
  registry-only pin projection with the original lock as preferences. It does not
  rebuild the projects, reconstruct their feature settings, or qualify git/path
  packages. Original source/license links are retained in `locks.json`.
- `frozen-top100.json`: unchanged 2026-11-13 roots, backed by the original CI sparse
  observations. The control requires 100 roots and exactly the historical 186-version
  union. It never receives ranking refreshes. The original fixture stays untouched.

`expected/*.json` records deterministic native output (including original index-line
hashes, reasons, features, collisions, warnings and refusals). `inputs/*.json.gz`
contains bounded, losslessly stored original sparse metadata for exact replay:

- `frozen-top100-20261113.json.gz`: 174 original observations from artifact
  `crates-top100-conformance`, run 43380261336; all observation hashes were checked
  against that artifact's frozen plan. Its hash is pinned in `frozen-top100.json`.
- `baseline.json.gz`: the coherent official sparse observations used for the
  initial current top/tricky/lock expectations; schema/date/source are inside.

Both bundles use deterministic gzip framing. They contain metadata only, never
crate archives, credentials, Cargo homes or certificates. Expected-file metadata
names the product source commit used to build the binary. Corpus refresh and
baseline proposals require reviewed PRs and full CI oracle proof; never auto-merge.
