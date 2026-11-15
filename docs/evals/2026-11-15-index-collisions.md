# Historical index collision patch — 2026-11-15

## Reproduction and policy

Published rust-mirror 0.5.0 was reproduced with a single bevy = "0.15" Windows
catalog: it failed before archive transfers or HEAD sizing with duplicate index
version, naming the Bevy root but not the offending crate. The culprit is ndk-sys:
non-yanked 0.4.0 and yanked 0.4.0+25.0.8775105 have different checksums. The
[original index fixture](../../tests/fixtures/ndk-sys-20261114.index) has SHA-256
767c4cf0391a7ed161587aa12735e71f138aea5fda8bb5488d13ee3e5ceaf555.
Cargo 1.90 directly resolved ndk-sys 0.6.0+11769913 with no archive requests.

Cargo's [index uniqueness rule](https://doc.rust-lang.org/cargo/reference/registry-index.html#version-uniqueness)
forbids normalized duplicates; this observed historical source nevertheless exists.
The owner-approved patch tolerates unselected observations, not conflicting selected
content. Normal candidate exclusions remain. Selected/manual/lock/retained normalized
identities with differing checksums refuse with typed context. No archive/index
identity is merged or repaired. The owner paused at 9de322e, then resumed and
explicitly authorized qualified 0.5.1 publication from the owner's machine.

## Local development conformance

The [dedicated oracle](../../scripts/check_crates_collisions.py) uses isolated Cargo
homes and full versus exact-byte filtered sparse indexes. Six cases passed during
development: historical live pair, historical yanked pair, Bevy-style Android chain,
selected-conflict refusal, original ndk-sys index and full-default winit 0.30.12 /
eframe 0.31.1 roots. Archive URLs are refused and no archive requests occurred.
Cargo may resolve the selected-conflict full index; rust-mirror deliberately refuses
it under the approved stricter checksum policy, rather than claiming equivalence.

The combined real windowing plan includes 397 versions, 370 index files and
25,671,577 metadata bytes. An initial research-default run hit its existing 2M-work
budget. The example's explicit production-bounds mode matches the catalog planner's
already-existing 20M work / 1M attempt limits; runtime defaults were not raised.
The repeated native plan succeeded at 3,250 attempts. This is resolution evidence,
not framework compilation or a full Bevy-default build claim.

Targeted tests cover selected/unselected metadata pairs, same-checksum identity
preservation, exact-version differing checksums, retained and lock-only guards,
frozen roundtrip and old plan/cohort golden identities. Initial targeted checks
and Clippy all targets passed; final exact-head checks are attached to
[PR #41](https://github.com/rustcommons/rust-mirror/pull/41/checks). Paused CI runs and
missing release-doc failures remain historical in that PR's resume comment.

## Frozen contracts and limits

Typed index_warnings are optional/empty-omitted in native/frozen/cohort/retention
observations, preserving legacy canonical hashes when empty. Nonempty observations
bind approval and historical replay. Selected-row checksum conflicts are also checked
against the proposed filtered index. Old strict readers reject new warning-bearing
plans; Rust callers constructing public structs need the added fields.

At most 1,024 warning groups, 32 members per group and 1,024 bytes per version are
accepted; budget/validation failures replace silent truncation. Existing unsupported
index semantics, native bounds and whole-index replay safeguards remain. No new
dependency, runtime Cargo, publication from this checkout or broader completeness
claim is introduced. Separate postpublication evidence is required after the owner
publishes. Existing package/client/native/Nexus gates remain mandatory.
