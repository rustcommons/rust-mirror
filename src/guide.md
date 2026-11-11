# rust-mirror: Rustup mirrors for offline development

Prepare selected Rust toolchains on a connected machine, transfer the verified
files into your offline network, and serve them through Nexus Raw or another
static HTTP host. Developers continue to use **Rustup** for installation and
updates. This crate provides both the `rust-mirror` command and a reusable library.

## Start here

1. [Install and choose platforms](#install-and-choose-platforms).
2. [Prepare a complete generation](#prepare-a-complete-generation).
3. [Publish and activate](#publish-and-activate).
4. [Configure Rustup clients](#configure-rustup-clients).
5. [Update or resume](#update-or-resume).
6. [Use the Rust library](#use-the-rust-library).

The [complete user guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/USER-GUIDE.md)
has Windows/Unix bootstrap and existing-client walkthroughs. The
[CLI reference](https://github.com/rustcommons/rust-mirror/blob/main/docs/CLI-REFERENCE.md)
lists every option, output field, budget and recovery rule.

## Install and choose platforms

Building requires Rust/Cargo 1.90 or newer. The installed executable does not
require Python:

```sh
cargo install rust-mirror --version 0.1.1 --locked
rust-mirror --help
rust-mirror list --toolchain 1.90.0 --host x86_64-unknown-linux-gnu
```

`list` downloads release metadata, not compiler archives. It reports availability,
not whether a native client was tested. The **host** is where the compiler runs;
a **target** is the environment for programs that compiler builds.

| Developer platform | Example host |
| --- | --- |
| Linux x86-64 with glibc | `x86_64-unknown-linux-gnu` |
| Windows x86-64 with MSVC | `x86_64-pc-windows-msvc` |
| Intel macOS | `x86_64-apple-darwin` |
| Apple Silicon macOS | `aarch64-apple-darwin` |

Select multiple hosts only when you need their archives. Native SDKs and linkers
remain client prerequisites; Rust distribution files do not include every OS SDK.

## Prepare a complete generation

Save this configuration as `mirror.toml` on a connected preparation machine:

```toml
schema = 1
rustup_version = "1.29.3"
max_download_bytes = 10737418240

[[selections]]
name = "linux-team"
toolchain = "1.90.0"
hosts = ["x86_64-unknown-linux-gnu"]
profile = "default"
components = []
targets = []
```

- `rustup_version` pins the installer/manager, separately from Rust `toolchain`.
- `toolchain` accepts full numeric releases, channels (`stable`, `beta`, `nightly`)
  and dated channels such as `stable-2025-09-18`. Saved selections do not accept
  partial numeric releases, local custom toolchains or arbitrary Rustup syntax.
- `hosts` explicitly chooses compiler platforms. Add `wasm32-unknown-unknown` to
  `targets` for that target's standard library; it is not a compiler host.
- `profile` is `minimal` or `default`. The default includes tools/docs selected by
  upstream. `components` adds named tools such as `rust-src`, `rustfmt` or `clippy`.
- `max_download_bytes` defaults to 10 GiB; it is a transfer/copy bound, not a disk
  quota. Allow disk space for working copies and the finished generation too.

```sh
rust-mirror sync --config mirror.toml --output ./run-001
rust-mirror rustup --snapshot ./run-001/mirror --version 1.29.3 --output ./generation-001
```

Use new output paths with existing parents. `sync` resolves every named selection,
freezes its metadata, verifies downloads and assembles a **raw generation**.
`rustup` copies it into a new **complete generation**, adding bootstrap/self-update
endpoints and numeric toolchain aliases. Neither command uploads files or installs
client tools. Host only **`generation-001/mirror`**, not private working state.

For explicit review of one frozen plan, the equivalent preparation stages are:

```sh
rust-mirror plan --config mirror.toml --selection linux-team --output ./plan-001
rust-mirror fetch --plan ./plan-001 --output ./snapshot-001
rust-mirror assemble --snapshot ./snapshot-001/mirror --output ./collection-001
rust-mirror rustup --snapshot ./collection-001/mirror --version 1.29.3 --output ./complete-reviewed
```

`plan` transfers metadata/checksums only; `fetch` uses that saved metadata without
selecting a newer release. Assembly combines completed inputs and refuses immutable
conflicts. For a dated channel, finishing may explicitly promote that included
channel with `--promote stable-2025-09-18`; numeric releases do not automatically
become the `stable` channel.

## Publish and activate

A Nexus administrator provisions fresh Raw hosted storage, HTTPS/read/upload
permissions and no-redeploy policy. Then the publisher runs:

```sh
rust-mirror publish --source ./generation-001/mirror --base-url https://nexus.example.local/repository/rust-generation-001
```

For private publisher trust add `--ca-file /path/to/company-ca.pem`. For Basic
authentication provide both `NEXUS_USERNAME` and `NEXUS_PASSWORD` through your secret
manager. Never put credentials in the URL or disable TLS validation. The library's
[`publish::Options`] takes credentials explicitly instead of reading environment
variables. Publisher trust does not configure Rustup client trust.

Publication validates local files, preflights that every remote path is absent,
uploads, verifies downloaded hashes and writes the inventory last. A failed upload
can leave partial remote data. It has no overwrite, resume or automatic retry mode.
Use a fresh prefix after investigation; never activate an incomplete generation.

**Activation is separate:** the operator exposes the verified hosted repository,
or changes a fixed Raw group's member order. The group routes paths; it does not
merge partial releases into a coherent toolchain. In-flight requests are not
atomically switched. See the
[Nexus guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/NEXUS-RAW.md)
for roles, group layout, MIME policy and TLS boundaries.

## Configure Rustup clients

On a client with Rustup already installed and the internal CA trusted, configure
the activated root. For Bash/sh:

```sh
export RUSTUP_DIST_SERVER=https://nexus.example.local/repository/rust-active/dist
export RUSTUP_UPDATE_ROOT=https://nexus.example.local/repository/rust-active/rustup
rustup toolchain install 1.90.0 --profile default --no-self-update
```

For Windows PowerShell:

```powershell
$env:RUSTUP_DIST_SERVER = 'https://nexus.example.local/repository/rust-active/dist'
$env:RUSTUP_UPDATE_ROOT = 'https://nexus.example.local/repository/rust-active/rustup'
rustup toolchain install 1.90.0 --profile default --no-self-update
if ($LASTEXITCODE -ne 0) { throw 'Rustup installation failed' }
```

The variables name distribution and manager-update roots respectively. They are
session-local in these examples; use your normal managed configuration for lasting
client settings. For fresh clients, obtain the verified mirrored `rustup-init`
installer for their exact host and follow the
[private-home bootstrap examples](https://github.com/rustcommons/rust-mirror/blob/main/docs/USER-GUIDE.md#bootstrap-a-windows-client).
Clients do not need rust-mirror installed. Offline Cargo projects still need their
crate/Git/native dependencies supplied separately.

## Update or resume

Build each next generation at a new path. `sync --previous OLD_MIRROR` reuses
verified local bytes and retains history. If nothing changed, it can return the
previous mirror path without creating the proposed new output. Use stdout's actual
result. Complete, publish, verify and explicitly activate the new generation before
asking clients to update. Keep pinned versions available for older clients.

| Situation | Rule |
| --- | --- |
| Interrupted selected download | `fetch --resume` with the same saved plan/output |
| Interrupted sync | `sync --resume` with the same config/previous/output identity |
| Deliberate restart of managed sync data | `sync --force`; destroys old managed output **without backup** |
| Existing ordinary output | Refused; select a fresh path |
| Failed upload | No remote force/resume; investigate and use fresh storage |

Resume does not refresh the frozen cohort. Missing payloads still require source
access; a frozen plan alone is not an offline archive bundle. Output locks must not
be bypassed by deleting lock files. Local force does not authorize remote overwrite.

Commands return 0 on success and 1 on errors; progress/errors use stderr and
paths/reports use stdout. Read errors before acting on directories left by failed
operations. Restoring older server data does not automatically downgrade installed
client tools.

## Command map

| Command | Purpose | Library route |
| --- | --- | --- |
| `init` | Write an example config | [`selection::write_config`] with your chosen [`selection::Config`] |
| `list` | Inspect release availability | [`selection::catalog`] |
| `import` | Convert a project toolchain file | [`selection::import_toolchain`] |
| `plan` | Resolve and save frozen metadata | [`selection::resolve`], [`selection::ResolvedPlan::save`] |
| `fetch` | Verify/download the frozen selection | [`selection::fetch`], [`selection::fetch_resuming`] |
| `sync` | Prepare all selections once | [`selection::sync::run`], [`selection::sync::run_forced`] |
| `assemble` | Retain/combine completed generations | [`selection::collection::assemble`] |
| `rustup` | Complete Rustup endpoints/promotion | [`selection::rustup::prepare`] |
| `publish` | Verify/upload fresh remote storage | [`publish::publish`] |
| `fetch-toolchain` | Legacy minimal GNU Linux snapshot | [`prepare`] |

## Use the Rust library

Add `rust-mirror = "0.1.1"` to your dependencies. Library use is synchronous; callers
choose paths, credentials, a transport and when to run each stage. A static config
can be parsed and validated without network access:

```rust
use rust_mirror::selection::Config;

let config = Config::parse(r#"
schema = 1
rustup_version = "1.29.3"
[[selections]]
name = "linux-team"
toolchain = "1.90.0"
hosts = ["x86_64-unknown-linux-gnu"]
profile = "minimal"
"#)?;
assert_eq!(config.get("linux-team")?.toolchain, "1.90.0");
# Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
```

Resolve and fetch one selection, then complete its Rustup endpoints. This example
compiles but is not run by doctests because it downloads and writes files:

```no_run
use rust_mirror::{selection, HttpsSource};
use std::path::Path;

let source = HttpsSource::default();
let config = selection::Config::read(Path::new("mirror.toml"))?;
let resolved = selection::resolve(&config, "linux-team", &source)?;
resolved.save(Path::new("plan-001"))?;
let snapshot = selection::fetch(Path::new("plan-001"), Path::new("snapshot-001"), &source)?;
let complete = selection::rustup::prepare(
    &snapshot, "1.29.3", &[], Path::new("generation-001"),
    config.max_download_bytes, &source,
)?;
println!("Serve {}", complete.display());
# Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
```

[`Source`] permits a caller-provided transport for controlled environments/tests.
Its URL, byte-budget and [`Transfer`] contracts are mandatory; returning bytes is
not proof that a payload matches its planned whole-file hash. [`HttpsSource`]
restricts upstream reads to the official HTTPS distribution origin.

For multiple selections use [`selection::sync`]. For completed local inputs use
[`selection::collection`]. For remote writes use [`publish`]. Inventory/report
types describe their fields and evidence boundaries on their API pages.

## Local retention (unreleased)

The source checkout adds local whole-generation pruning for the planned 0.2.0
feature; published 0.1.x does not include it. Offline clients cannot report which
releases they need, so declare pins and stable/nightly/rollback counts explicitly.
`sync` never invokes pruning. Preview reads/hashes supplied schema-2/3 roots and
returns structured file sizes/paths and keep reasons without printing or writing.

```no_run
use rust_mirror::selection::{Config, retention};
use std::path::{Path, PathBuf};
let config = Config::read(Path::new("mirror.toml"))?;
let policy = config.retention.as_ref().ok_or("configure [retention] first")?;
let roots = vec![PathBuf::from("generation-002/mirror"), PathBuf::from("generation-001/mirror")];
let plan = retention::preview(policy, &roots, "2025-09-22")?;
// Present plan.files, total_bytes and kept reasons in the GUI, then obtain approval.
let user_approved = false;
if user_approved {
    let report = retention::apply(&plan, true)?;
    if let Some(failure) = report.failure {
        eprintln!("Stopped at {}: {}", failure.path.display(), failure.message);
    }
}
# Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
```

For directory-based GUI screens, `retention::discover(directory)` returns a
metadata-only serializable `Catalog`. `retention::preview_directory(policy,
directory, next_previous, as_of)` protects the exact next-sync anchor and paused
run dependencies, and binds managed files/private state and membership to apply.
`PrunePlan::folders` provides per-folder logical totals. Locked/unknown/incomplete
runs are never deletion candidates; private workspace deletion and common
progress/cancel/events support remain deferred.

Current first is always protected; following roots are declared newest-first.
Apply requires explicit authority and revalidates the preview under advisory locks.
It removes whole roots only and returns successful-prefix information on I/O failure;
there is no rollback, live-reader protection, in-generation compaction or Nexus
DELETE. Keep trusted private parents and pause external users/writers before apply.
GUI callers should run synchronous hashing/deletion off the UI thread. Existing
Rust `Config` literals need `retention: None` or an explicit policy; existing TOML
without the table remains valid. See the
[retention contract](https://github.com/rustcommons/rust-mirror/blob/main/docs/RETENTION.md).

## Safety, scope and further reading

- Original upstream manifests and signature files are preserved. HTTPS/upstream
  hashes and trusted inventories are the trust model; independent OpenPGP
  verification is not implemented.
- Native lifecycle evidence covers recorded versions on GNU Linux x86-64, Windows
  MSVC x86-64 and both macOS architectures. Other advertised hosts remain unqualified.
- Actual Nexus evidence used isolated legacy OSS 3.76.1-01. Your production policy,
  current CE onboarding, authentication and proxy configuration require assessment.
- There is no Cargo registry mirroring, SDK provisioning, scheduling, Nexus pruning or GUI.
  No hostile-filesystem, NFS, power-loss, globally atomic activation or arbitrary
  historical/custom-toolchain guarantee is implied.

See the [user guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/USER-GUIDE.md),
[complete CLI reference](https://github.com/rustcommons/rust-mirror/blob/main/docs/CLI-REFERENCE.md),
[qualification](https://github.com/rustcommons/rust-mirror/blob/main/docs/evals/2026-11-11-rustup-lifecycle.md)
and [changelog](https://github.com/rustcommons/rust-mirror/blob/main/CHANGELOG.md).
