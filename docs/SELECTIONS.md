# Select Rustup toolchains and platforms

This experimental workflow chooses what to mirror before downloading archives.
It is not a Rustup replacement. Rustup remains the client installer.

## Start with a saved selection

Build with Rust/Cargo 1.90 or newer, then create a new configuration file:

```sh
cargo build --locked
rust-mirror init --config mirror.toml
```

Use `target/debug/rust-mirror` (Windows: `target/debug/rust-mirror.exe`) if the
binary is not on PATH. `init` writes the [example](../examples/mirror.toml),
including editable Linux, Windows and macOS selections. It downloads nothing.

```toml
schema = 1
rustup_version = "1.28.2"
max_download_bytes = 10737418240

[[selections]]
name = "project-linux"
toolchain = "1.90.0"
hosts = ["x86_64-unknown-linux-gnu"]
profile = "minimal"
components = ["clippy", "rustfmt", "rust-src"]
targets = ["wasm32-unknown-unknown"]
```

A **host** is where the installed compiler runs. A **target** is where its output
will run. Adding `wasm32-unknown-unknown` downloads that target's standard library;
it does not provide a linker, SDK, system libraries or an execution environment.
Multiple hosts in one selection share the same requested profile/components/targets.
An unavailable explicit request fails the whole plan rather than silently omitting it.
Use separate named selections when different hosts need different tools.

`toolchain` accepts exact numeric versions (`1.90.0`), `stable`, `beta`, `nightly`,
and dated channels such as `nightly-2025-09-18`. Partial numeric versions (`1.90`),
versioned beta selectors and custom/local toolchains are not supported yet.
Rustup's bootstrap version is independently pinned. Cargo, Clippy and other
components come from the selected Rust manifest; independent incompatible
component-version mixing is not offered.

`profile` defaults to `default`; `minimal` is also supported. Profiles are read
from the selected release rather than hard-coded. `complete` is refused: choose
`default` plus explicit components instead. The [Rustup profile documentation](https://rust-lang.github.io/rustup/concepts/profiles.html)
explains why `complete` is unreliable. Nightly tools can be absent on particular
dates; the planner does not search backwards for an alternative date.

## Discover release-specific availability

```sh
rust-mirror list --toolchain 1.90.0
rust-mirror list --toolchain 1.90.0 --host x86_64-unknown-linux-gnu
```

Output is TOML. Without a host it lists host availability and profiles. With a
host it also lists host/global components, aliases, package-version text and
compilation targets. An availability flag describes upstream metadata, not native
installation or runtime qualification. Consult `supported_profiles`, since upstream
metadata may also advertise the unsupported `complete` profile. Listing does not
fetch installers, so it does not prove a bootstrap exists; planning checks that
bootstrap's checksum endpoint too.

## Import a project's toolchain file

```sh
rust-mirror import --toolchain-file ./rust-toolchain.toml --name project-linux --hosts x86_64-unknown-linux-gnu --rustup-version 1.28.2 --output project-mirror.toml
```

Imports `channel`, `profile`, `components` and `targets` from `[toolchain]`.
The older ASCII `rust-toolchain` selector file is supported too. Hosts and the
Rustup version must be explicit: a project file does not establish either one.
Unknown fields, missing channel, local paths and unsupported selectors fail.
There is no implicit parent-directory search, environment override or host inference.
Existing output files are never overwritten.

## Preview and freeze, then fetch

```sh
rust-mirror plan --config project-mirror.toml --selection project-linux --output ./project-plan
rust-mirror fetch --plan ./project-plan --output ./project-snapshot
```

`plan` downloads only the manifest, its checksum/signature files and selected
bootstrap checksums. It writes the selected artifact paths/hashes and cached
metadata into a fresh plan directory; TOML preview goes to stdout. Diagnostics
and the download budget go to stderr. Archive sizes are **unknown** at planning
time. `fetch` re-derives and checks that plan against the cached bytes, then fetches
only its frozen artifact URLs; it does not refresh live channel metadata.

For moving channels the plan records the resolved release and a dated install
selector, for example `stable-2026-10-15`. Use the actual `pinned_toolchain` from
your plan rather than copying this date. Resolve a new plan explicitly to update.
One plan/snapshot represents one named selection and one resolved Rust release;
multiple versions require separate snapshots/hosting prefixes at present.

All advertised gzip/xz/zstd variants for a selected component are included.
Rustup clients may choose different formats; preserving an original manifest while
omitting a client-selected format would break offline installs. This costs more
storage than downloading just the preferred format. Duplicate paths are downloaded
once; aliases such as `clippy` follow manifest renames and global `rust-src` is
included through the wildcard target.

The completed tree is `project-snapshot/mirror`. It preserves upstream metadata
bytes and adds the dated lookup path pointing to those same bytes. `snapshot.toml`
uses **schema 2**, recording the selection, date, pinned selector and file hashes.
The legacy `fetch-toolchain` command still creates schema 1 minimal Linux snapshots.

## Host and install

The [publisher and Nexus setup](NEXUS-RAW.md) applies to both inventory schemas.
Use a new, empty hosting prefix per snapshot. The publisher verifies archives
before publishing root/dated metadata and the inventory marker last. There is no
multi-version merge, transaction across a live server, or overwrite/prune operation.

Set `RUSTUP_DIST_SERVER` to the snapshot's root and `RUSTUP_UPDATE_ROOT` to its
`rustup` subdirectory. Download the pinned host bootstrap from
`rustup/archive/VERSION/HOST/rustup-init` (`rustup-init.exe` for Windows), verify it
against its sidecar, and initialize with no default toolchain. Disable automatic
Rustup self-update. Then install the plan's `pinned_toolchain` with the same profile,
components and targets. See the [bootstrap/client commands](NEXUS-RAW.md#install-on-the-disconnected-linux-machine).
Unselected components and arbitrary update requests are not supplied by the mirror.

The current real-Nexus harness is deliberately limited to its original schema 1
case. Schema 2 publication/install is tested through the local HTTP contract fixture,
not a new Nexus product qualification. See [actual Nexus boundaries](NEXUS-INTEGRATION.md).

## Limits and trust

- Strict schema 1 configuration: 1?32 unique named selections, 1?16 unique hosts per
  selection, at most 64 explicit components and 64 targets. Identifier/path checks apply.
- At most 1,024 payload files; manifests up to 4 MiB; config/plan up to 1 MiB;
  signatures up to 64 KiB; individual downloads up to 512 MiB.
- Download budget defaults to 10 GiB and can be set from 1 byte through 100 GiB.
  It counts payload and copied metadata bytes, not generated inventory/filesystem
  overhead. All-format downloads may exceed it; failure exposes no completed tree.
- New output paths only; their parent must exist. Use private, trusted parent
  directories. An interrupted plan or failed fetch may leave an incomplete output
  directory; select a fresh path when retrying. No resumable/shared cache, hostile
  filesystem-race protection or concurrent publication guarantee is claimed.
- Only official `https://static.rust-lang.org/` downloads are accepted. SHA-256 and
  original signature files are retained; independent OpenPGP verification is not
  implemented. Plan hashes detect inconsistency, not an attacker rewriting a whole
  plan. Treat transferred plans/snapshots as trusted administrative inputs.
- Real execution is demonstrated only for the recorded Linux GNU toolchain and
  selected components. Windows/macOS/musl/ARM availability checks do not qualify
  native installation. Native prerequisites, Cargo crate mirroring, GUI, scheduled
  synchronization, retention and Rustup self-update remain outside this iteration.

## Offline proof

For an already downloaded native GNU snapshot, run the bounded disposable proof
inside a fresh Linux network namespace, never the user's Rustup/Cargo homes:

```sh
sudo unshare --net python3 scripts/prove_offline.py --snapshot /absolute/project-snapshot/mirror --evidence /absolute/new-evidence
```

For a multi-host snapshot add `--host x86_64-unknown-linux-gnu`. The proof refuses
other execution hosts. It checks installed tools/targets, builds/runs a small offline
program, runs installed Clippy/rustfmt and typechecks Wasm metadata when selected.
Missing/corrupt controls cover every archive variant for one actually downloaded
component. The [dated record](evals/2026-11-09-toolchain-selection.md) states exact
versions, results and remaining work.
