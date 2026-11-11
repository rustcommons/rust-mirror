# Rustup-complete 0.1.0 release gates

Owner direction, 2026-11-11: complete Rustup mirror functionality and actual Nexus
qualification are required before 0.1.0. Scheduling is excluded. No registry
publication is authorized by this implementation request.

## Required workflows

- Explicit Linux, Windows and macOS hosts; exact/datestamped/stable/beta/nightly
  selections; available profiles, components and compilation targets.
- Frozen planning, verified all-format transfers, resume/reset, multi-version
  retention, original manifests and isolated homes.
- Pinned Rustup bootstrap plus self-update catalog, archived/current installers,
  original shell installer, and explicit promotion of dated channel metadata.
- Fresh native client bootstrap/install, component and target add/remove/re-add,
  toolchain remove/reinstall, offline project use, existing stable-client update,
  unchanged repeat, Rustup self-update and already-current self-update; install the
  default profile and resolve its actual offline documentation index.
- Actual Nexus Raw schema 2 and 3 publication/GET verification, TLS/private CA,
  scoped writer/read-only clients, no-redeploy, missing/corrupt controls and an
  existing-client old/new generation update through a fixed Raw group URL. No production server writes.
- Native Windows/x86-64, macOS/Intel+Apple-Silicon and Linux/x86-64 proof jobs, with external-access isolation controls
  recorded separately from ordinary endpoint-redirection checks.
- A repeatable demonstration, final package/install qualification, release notes,
  changelog, registry/tag/docs.rs/Release agreement before release closeout.

## Boundaries

Complete means the selected, advertised Rustup artifacts and normal client lifecycle,
not all historical artifacts downloaded indiscriminately, unsupported upstream hosts,
C/C++ SDK provisioning, Cargo registry mirroring, GUI, scheduling or retention pruning.
Unavailable explicit upstream requests must fail rather than silently shrink.

Independent signature verification remains an explicit trust gate to assess, not
an implied consequence of HTTPS/checksums. Current Community Edition license
onboarding remains an operator decision; tests must never accept an EULA.

This is a gate checklist, not a completed qualification record. Keep every gate
open until its exact source, native results and server evidence are recorded.

## Current disposition

All implementation, native client and actual Nexus gates above passed at qualified
`4738aef` / implementation merge `7c020b5`. See the
[final prepublication record](evals/2026-11-11-rustup-lifecycle.md#final-prepublication-qualification)
and [structured evidence](evals/rustup-0.1.0-qualification.json).

The documentation-complete 0.1.0 crate is published from clean source
`93f38921000ba4e8cc1d55006afd326c077d8f7e`. Official archive/API/index hashes,
docs.rs and fresh crates.io installation/installed-publisher checks passed.
The annotated `v0.1.0` tag identifies that exact source. See the
[publication record](evals/2026-11-11-publication.md) for actual results and the
[release notes](releases/v0.1.0.md). Historical qualification below keeps its
original scope; no broad native/Nexus matrix was repeated for publication.
Scheduling and the other stated exclusions remain outside 0.1.0.

## Documentation expansion before publication

The owner's full-documentation request adds the user guide/reference and changes
packaged README/changelog bytes. The earlier frozen `0b38e459` archive is superseded
for publication. Code, manifest, lock, examples and license are unchanged, so the
native/server qualification carries in its recorded scope. Final clean packaging passed at `93f38921000ba4e8cc1d55006afd326c077d8f7e`: 18 files,
49,142 bytes; only README/changelog/VCS differ from the earlier archive.
Do not upload the superseded archive.
