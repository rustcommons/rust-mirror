# Explicit sync reset, 2026-11-10

The owner requested refusal without an explicit choice for an existing run, and
specified that force must overwrite without a backup. `sync --force` now removes
only the managed `.sync` progress and `mirror`, retaining the single-writer lock
while resolving and rebuilding. See [the current contract](../SYNC.md#explicit-restart-with---force).
`--resume` continues the original cohort; combining the flags or repeating either
refuses before I/O. Other commands retain their existing contracts.

Validation on the implementation delivered in PR #11:

- Windows: 67 Rust tests passed, including a real junction refusal control.
- Ubuntu/WSL Rust 1.90: 71 Rust tests passed, including symlink/FIFO refusal.
- Both: fmt, Clippy with warnings denied, and warnings-denied rustdoc passed.
- Windows: 12 Python tests passed, with one existing symlink-privilege skip.
- Real PowerShell fixture demo: explicit existing-output error, resume, unchanged,
  then force creation with no backup. Synthetic payloads only; no downloads or
  toolchain installation in this follow-up.
- Destructive reset tests cover interrupted/completed runs, old-file removal,
  active-lock and unrelated-directory refusal, previous overlap and external
  previous preservation/reuse, failed fresh resolution, and lock-only retry.

The Windows junction fixture initially used a mixed-separator command operand;
that fixture failed before creating the junction. Native-component paths corrected
it, and the actual junction control and final suite passed. An initial attempt to
capture the PowerShell demo through PowerShell's stderr redirection terminated on
Cargo's diagnostic stream; direct process capture passed. Neither is counted as
successful evidence. Earlier whole-directory-backup experiments are superseded by
the owner's destructive-reset contract.

The earlier real-data demo remains historical evidence of sync, not a test of
force. No new Nexus, rustup install, foreign-host or power-loss qualification is
claimed. Reset is destructive and not transactional; failure may require another
`--force`. Trusted local parents and filesystem locks remain required. No release,
crate upload, dependency/version/lock change, or merge occurred in this follow-up.
