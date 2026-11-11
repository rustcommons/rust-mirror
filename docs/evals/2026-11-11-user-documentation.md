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
archive remains historical evidence and is superseded for publication. The refreshed
clean package/VCS audit is recorded below; use that documentation-complete candidate
for any later authorized upload. Native/Nexus evidence can carry by unchanged compiled source and dependency
inputs; do not label it as a new runtime matrix. Publication remains on hold for
the owner's go-ahead. Local detailed validation receipts are in this thread's
`rust-mirror-user-docs-20261111` storage directory.

## Documentation-complete clean package

PR15 merged as `93f38921000ba4e8cc1d55006afd326c077d8f7e`, with its tree identical to reviewed
`bbbfa090b919f3667abf9ba7b5fe1989b410460b`. Both Rust 1.90 Linux/Windows CI jobs
and release-documentation checks passed. The initial Linux run failed to reacquire
a workspace lock in an existing recovery test; the unchanged-source rerun passed.
The saved failed log remains evidence, not a claim that its cause was fixed.

Rust/Cargo 1.90 `cargo package --locked` built and verified the exact clean merge.
The archive has 18 files and 49,142 bytes, SHA-256:

```text
b6e01cc29cefc18121c54c23230238a076c9935b8823523073021f8cdba7a69c
```

All included repository payloads match that commit. Compared with the previous
`0b38e459` archive, only README, changelog and VCS metadata differ. Source,
normalized/original manifests, lock, examples and license are byte-identical.
The actual package verification compiled the new archive; no fresh unpacked
installation or native/Nexus matrix was repeated for these documentation changes.
Earlier installation/runtime evidence carries only for unchanged executable and
dependency inputs. This is the current frozen candidate; later excluded-document
bookkeeping does not change its package inputs. No publication, tag or Release
has been performed. Official postpublication verification remains pending.

## Published-state follow-up

The owner subsequently authorized publication. The exact documentation-complete
archive recorded above was uploaded unchanged and independently downloaded from
crates.io. See the [publication record](2026-11-11-publication.md) for official
checksums, actual registry-install evidence and release verification. This does
not rewrite the prepublication hold or claim new broad runtime qualification.
