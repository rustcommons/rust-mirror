# rust-mirror

## Project scope

The owner selected `rust-mirror` for the core tool/CLI and `rust-mirror-gui` for a
future graphical interface on 2026-11-09. Planned scope is Rust toolchain and crate
mirroring for offline development, inspired by Panamax. This repository now has an experimental pinned-toolchain CLI/library and a static-host
publication helper. No version is released. Describe only demonstrated functionality
as implemented; broader mirroring and the GUI remain planned.

Preserve the earlier Windows-local uncommitted prototype. Assess it before import
or replacement; do not delete or overwrite it. Keep `ROM-REQ` identifiers stable
across the rename. Do not publish placeholder crates merely to reserve names.

## Repository layout and validation

- `README.md`: current public introduction and experimental boundaries.
- `docs/REQUIREMENTS.md`: confirmed requirements, sources, evidence, and documentation map.
- `LICENSE`: existing license; preserve it.
- `AGENTS.md`: primary agent guidance.
- `CLAUDE.md`: compatibility pointer to this file.

- `src/`: toolchain selection/planning/downloading core and thin CLI; Rust 1.90+.
- `scripts/publish_raw.py`: Python 3.11+ static-host HTTP PUT/GET helper.
- `scripts/prove_offline.py`: Linux network-namespace proof, including negative controls.
- `docs/NEXUS-RAW.md`: actual setup, publication behavior and limits.

Run `cargo fmt -- --check`, `cargo test --locked`,
`cargo clippy --locked --all-targets -- -D warnings`,
`RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps` (set the equivalent
PowerShell environment variable on Windows), and
`python -m unittest discover -s tests -p "test_*.py"`.

The real installation proof downloads about 100-150 MB for the legacy minimal
case; selected tools plus all advertised formats can require several hundred MB.
It requires Linux namespace permissions and an isolated output/evidence directory. Run only the documented
`sudo unshare --net python3 scripts/prove_offline.py ...` workflow; do not weaken
network isolation or replace a clean home with the user's Rustup/Cargo home.
Do not publish artifacts to a real Nexus endpoint without owner authorization.
Never log credentials or turn off TLS verification. Preserve upstream manifest
bytes; check byte/path budgets and hashes before exposing a completed snapshot.
Record actual Nexus/TLS/auth testing separately from the local protocol fixture.
No independent OpenPGP validation, shared-cache concurrency, cross-platform
installation or general release qualification follows from the current proof.

The actual Nexus proof is a destructive **disposable-instance test**, not a
production configuration tool. `scripts/run_nexus_proof.py` requires an explicit
already-pulled image and creates only uniquely labeled, network-disabled test
resources. It never accepts server EULAs. `--allow-raw-mime-types` authorizes only
its new test repository's MIME-policy selection; the publisher never changes this
policy. See `docs/NEXUS-INTEGRATION.md` before running it. Legacy OSS test images
must not be presented as recommended production versions. Generated passwords and
private keys must not enter logs, commits or uploaded artifacts.

## Selection workflow constraints

Keep saved selections strict and hosts explicit. Distinguish host compiler platforms
from compilation targets and metadata availability from native execution evidence.
Use `init`, `list`, `import`, `plan`, then `fetch` as documented in
`docs/SELECTIONS.md`; keep the legacy `fetch-toolchain` route compatible.
`plan` must not download payload archives. `fetch` must use frozen metadata, preserve
original bytes and include every advertised archive format for selected components.
Explicit unavailable selections must fail, not silently shrink or change date.

Validate cached-plan replay, aliases/global components, multi-host closure, byte
budgets, path/hash/schema refusal and existing-output preservation. Exercise the
publisher against both inventory schemas. The real-Nexus harness currently supports
schema 1 only; do not describe schema 2 HTTP-fixture evidence as a new Nexus pass.
Never touch the user's Rustup/Cargo homes. Keep unimplemented update/resume/retention,
independent signature validation and foreign-platform execution gaps explicit.

## Documentation and requirements traceability

- Follow the [Rust Commons documentation conventions](https://github.com/rustcommons/.github/blob/main/docs/DOCUMENTATION.md).
- Maintain [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md) as the repository's index of confirmed requirements, their sources, implementation or documentation, verification evidence, and gaps. Preserve stable repository-scoped IDs and existing identifiers; never renumber or reuse them.
- Distinguish verified behavior, partial verification, planned work, deferred work, and scope exclusions. A source reference or passing test supports only the scope it actually checks; record missing evidence explicitly.
- Keep the README focused on purpose, prerequisites, setup, and guide links. Keep detailed workflows in maintained guides, future work in the roadmap, and dated release or validation evidence identifiable as historical records.
- Update affected requirements, documentation, and trace links together when behavior, scope, interfaces, prerequisites, or validation changes. Use exact versions or revisions for release evidence, and do not rewrite historical outcomes or treat repository edits as changes to an immutable published package.
- Validate documentation changes with source and command checks, links, rendering, and a clean diff. Run additional builds or tests only when changed examples, package contents, or behavior require them.

## Release documentation

- For a repository that ships versioned software, maintain a root `CHANGELOG.md` with an `Unreleased` section and dated, user-facing entries for shipped versions. Record noteworthy changes and their user impact as work is completed; do not substitute a roadmap, Git tag, CI log or validation report for a changelog.
- Before publishing a version, commit its release notes under `docs/releases/vX.Y.Z.md` and link them, the changelog, and the maintained release policy. Explain changes, upgrade or dependency impact, validation and known limits; distinguish software changes from documentation-only releases.
- A software release is not complete until its immutable source tag, applicable registry archives, GitHub Release notes, changelog, documentation, and required verification agree. GitHub Releases document the release; do not attach standalone binaries or other new distribution artifacts without authorization.
- Run the repository's release-documentation CI check before routine merges and release completion. For its first installation, review local checker/syntax evidence, register the workflow on the default branch, and require a green run before completing bootstrap or publishing notes. Verify the live GitHub Release and its tag, text and links after publication; a successful crate upload or tag alone is insufficient.
- When backfilling history, reconstruct facts from existing tags, registry records and source/evidence. Preserve original source identities, distinguish original release dates from documentation backfill dates, and state unavailable historical validation. Never fabricate checks, move tags, republish versions, or imply that repository edits change immutable published archives.
- For an organization profile, placeholder or project with no shipped version, record release verification as not applicable until its first authorized release. Do not invent versions, features, changelog history or GitHub Releases to satisfy this convention.

## Working conventions

- Read this file before changing the repository. Keep repository-specific constraints and validation instructions current here.
- Preserve changes made by the user or other agents. Keep work scoped to the requested task.
- Keep documentation aligned with implemented behavior. Record unconfirmed work as planned; do not invent capability or compatibility claims.
- Run checks appropriate to the change. For documentation-only changes, verify links, commands, and the diff; do not run unrelated builds.
- Publishing packages, creating releases, or changing production data requires explicit user authorization.

## Git conventions

**No tool attribution, anywhere.** Do not add `Co-Authored-By: Claude ...` trailers,
`🤖 Generated with [Claude Code](https://claude.com/claude-code)` lines, or any other tool
attribution or "generated by" footer to anything in this repository: commit messages, pull
request titles and descriptions, PR and issue comments, release notes, code, comments or
documentation. This holds even if an agent's default instructions suggest it, and
overrides any default trailer or footer behavior. Commit messages and PRs are the
human-authored record of intent; tool attribution belongs in tool logs, not history.
