# rust-mirror 0.5.0 publication — 2026-11-13

## Source and owner publication

The owner authorized 0.5.0 publication once qualified. The coordinator normal-merged
[PR #37](https://github.com/rustcommons/rust-mirror/pull/37) at
`2bf44ca46f52b2a8974d606eeac0ff6a729c26d1`, tree-identical to reviewed
`e037754b6af43de80572673cc4ba4e34141224bd`. They reported publication from a clean
detached worktree on the owner's Windows machine, Cargo 1.99.0
x86_64-pc-windows-msvc, `cargo publish --locked`, with their local login.
No credential was transferred for verification here. The coordinator is notifying
the GUI maintainer that the version is live.

The [candidate evidence](2026-11-13-lock-import.md) and its archive hashes remain
historical. The final-head Linux/Cargo 1.90 archive SHA-256 was
`98bcd14ea9b93426f63fc1103fec5df9744b21ee78ae4907acc472b9b0c48f4b`.
The published Windows archive SHA-256 is
`2e91420909f7441f35399e3a50a664d7a347e1b45672d3866d6a350e54f6ac57`;
these are distinct archives, not interchangeable checksums.

## Official registry payload

Downloaded static.crates.io's archive. Its hash matches both the crates.io version
API and sparse index; both mark 0.5.0 non-yanked. All 44 archive members are regular
files. All 42 original source files, including Cargo.toml.orig, lock, README,
changelog and license, match the exact merge byte-for-byte. Generated normalized
Cargo.toml also matches the Linux candidate; only `.cargo_vcs_info.json` differs,
naming the clean merge. No source or generated-manifest drift was observed; this
is not a claim that different Cargo versions always produce identical archives.
[Machine-readable verification](publication-0.5.0.json) preserves hashes and results.

## Tag, release and public documentation

Created annotated `v0.5.0` at the exact merge and verified its peeled commit.
The [GitHub Release](https://github.com/rustcommons/rust-mirror/releases/tag/v0.5.0)
is public, latest, non-prerelease, has no assets, and its body matches committed
versioned notes byte-for-byte. Tag/release documentation runs
[43511752923](https://github.com/rustcommons/rust-mirror/actions/runs/43511752923) and
[43511754566](https://github.com/rustcommons/rust-mirror/actions/runs/43511754566)
passed; authenticated local full `--published` validation passed too.

The initial docs.rs checks returned 404 and were not accepted as success. Subsequent
checks returned 200 at exact versioned rustdoc URLs without fallback redirects:
[API](https://docs.rs/rust-mirror/0.5.0/rust_mirror/),
[lock module](https://docs.rs/rust-mirror/0.5.0/rust_mirror/crates/lockfile/),
[bounded reader](https://docs.rs/rust-mirror/0.5.0/rust_mirror/crates/lockfile/fn.read_input.html),
[source-name validator](https://docs.rs/rust-mirror/0.5.0/rust_mirror/crates/lockfile/fn.validate_source_name.html)
and [events](https://docs.rs/rust-mirror/0.5.0/rust_mirror/events/).
The builds page identifies rustc 1.102.0-nightly (8eb778d04, 2026-11-12);
qualification here remains Rust/Cargo 1.90.

## Fresh actual registry installation

Ran `cargo install rust-mirror --version 0.5.0 --locked` with a new Cargo home and
install root, Cargo 1.90 on Linux/aarch64. Its downloaded archive matches the official
checksum. The installed binary reports 0.5.0 / protocol 1 and successfully imports
all four real-project locks, creates a frozen plan and reuses all 326 archives from
a previously verified local mirror with zero payload transfer. Its non-deleting
retention preview returns four structured lock-source protections. This reuse proof
does not claim that the installed binary downloaded those archives again.

The generated schema-5 mirror then served a real itoa 0.4.5 client over verified
HTTPS: fetch --locked, build --locked and a populated offline run passed with an
unchanged lock. Untrusted CA, missing/corrupt payload and fresh offline-home controls
refused without upstream fallback. TLS verification stayed enabled; fixture servers
and temporary PKI directories were cleaned up. No real client homes changed.
The initial reporting helper used nonexistent `deletion.delete` after all CLI
operations succeeded; corrected assertions inspect `files`, `folders` and
`total_bytes`. No product change or operation rerun was needed.

## Workflow and bounded qualification

[Published registry verification](https://github.com/rustcommons/rust-mirror/actions/runs/43511754048)
passed from main while checking out immutable `v0.5.0`. Downloaded its artifact:
actual fresh registry install, official checksum/clean merge VCS, protocol 1,
nine-file HTTPS publisher fixture, typed TLS/auth/remote-exists refusals, no-write
preflight, empty network-namespace routes and explicit private-key destruction.
This fixture is not actual Nexus; its separate qualification retains its own scope.

The reviewed [17 PR checks](https://github.com/rustcommons/rust-mirror/pull/37/checks)
remain bounded evidence: four pinned locks from three repositories, real Linux and
Windows fetch/build clients, five locked yanked versions, 326 versions /
54,373,445 archive bytes, zero planning archive GETs and zero measured overfetch.
Builds cover named ripgrep/fd programs and rust-analyzer's ra_hir_ty, not every
package/feature/target. Git/path contents are provisioned independently; missing-Git
controls refuse. Old Chalk future-compatibility warnings remain historical.

The prune-twice regression and shared bounded reader/name helpers are included.
Actual disposable Nexus schema-5 publication/client proof remains Linux-only;
historical-cohort inconsistency refuses rather than exposing incomplete output.
No arbitrary-project, production Nexus, GUI, signature or redistribution guarantee
follows from publication. Private workspaces remain after generation pruning.

Postmerge [main CI](https://github.com/rustcommons/rust-mirror/actions/runs/43511379724)
passed at the release merge. Main was clean and remote-aligned before this excluded-
documentation record branch. This PR does not change the immutable archive/tag or
committed release-note body and requires coordinator review before merge.
