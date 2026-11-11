# Full user documentation, 2026-11-11

Owner request: document all current functionality so terminology and operation are
understandable before 0.1.0 publication. This is documentation validation, not new
runtime or production-Nexus qualification.

## Scope

- [User guide](../USER-GUIDE.md): terms, three separate operational roles, platform
  selection, complete old/new generation workflow, Windows/Unix clients, retained
  pinned versions, self-update, recovery, offline boundaries and practical limits.
- [Reference](../CLI-REFERENCE.md): all ten CLI commands, 36 command-option pairs,
  four global forms, configuration fields, environment/trust, output data/report
  fields, budgets, inventory schemas and public library entry points.
- README entry points, deep-guide navigation, [requirements](../REQUIREMENTS.md)
  and repo-specific AGENTS maintenance instructions updated together. Existing
  shared instruction blocks and requirement IDs remain unchanged; ROM-REQ-030 is
  the new owner documentation requirement.

## Checks and evidence boundaries

Source/CLI help coverage was audited against `src/main.rs`; defaults, serialization
fields, budgets and library names were checked against the implementation. This
caught and removed an invented sync-report schema field. The trace table was also
made contiguous so all requirement rows render as an actual GitHub table.

Four PowerShell examples passed the built-in syntax parser. Eighteen Bash/sh
examples passed `bash -n`. Two complete tutorial configurations passed the actual
library's `Config::read` validation without network access. TOML fragments were
parsed, and the library Rust example compiled with Rust 1.90 against the qualified
library. That executable was not run: it would intentionally perform network and
local writes. Relative links/anchors, GitHub Markdown rendering, JSON structure,
shared-instruction preservation and diff checks were reviewed.

These checks do not assert that every newly written deployment snippet was executed
on an arbitrary workstation or production Nexus server. The actual client/server
runtime evidence is separately bounded by the
[existing qualification record](2026-11-11-rustup-lifecycle.md). No credentials were
collected, production writes performed, EULA accepted or new project dependency
added. Source, manifest, lock, examples, license, scripts and workflows are unchanged.

## Package and release disposition

Only README/changelog change within the distributed crate. The earlier `0b38e459`
archive remains historical evidence and is superseded for publication. Refresh the
clean package/VCS audit and freeze the documentation-complete candidate before
upload. Native/Nexus evidence can carry by unchanged compiled source and dependency
inputs; do not label it as a new runtime matrix. Publication remains on hold for
the owner's go-ahead. Local detailed validation receipts are in this thread's
`rust-mirror-user-docs-20261111` storage directory.
