# rust-mirror user guide

Start here to understand and operate published rust-mirror 0.1.1. Install with
`cargo install rust-mirror --version 0.1.1 --locked`.

This guide explains the normal workflow. The [command reference](CLI-REFERENCE.md)
lists every command/option, defaults, outputs and failure behavior. The
[qualification record](evals/2026-11-11-rustup-lifecycle.md) distinguishes tested
versions/hosts from general metadata availability.

Read in order for a first deployment, or jump to:

- [Concepts](#words-used-in-these-guides) and [platform/tool selection](#choose-versions-platforms-and-tooling)
- [First generation](#worked-example-first-generation) and [files on disk](#files-on-disk)
- [Nexus publication/activation](#nexus-publication-and-activation)
- [Windows client](#bootstrap-a-windows-client) or [Linux/macOS client](#bootstrap-linux-or-macos-clients)
- [Next generation](#prepare-the-next-generation) and [updating the same client](#update-that-same-existing-client)
- [Resume/restart](#resume-restart-and-alternative-preparation) and [troubleshooting](#results-and-troubleshooting)
- [Scope and further reference](#limits-demos-and-further-reading)

## What the tool actually does

Run rust-mirror on a machine with internet access. It downloads the Rust files
you select, verifies them, and produces a directory suitable for static hosting.
Transfer that directory to an offline network and publish it into Nexus Raw.
On developer machines, **Rustup remains the installer and updater**. Developers
do not need rust-mirror installed just to use the hosted files.

Publishing a mirror is different from publishing this tool: `rust-mirror publish`
uploads distribution files to Nexus/static hosting. It never publishes a crate
to crates.io; `cargo publish` is a separate maintainer release action.

| Job | Who performs it | What changes |
| --- | --- | --- |
| Prepare a generation | rust-mirror on the connected machine | New local files; upstream is read only |
| Publish a generation | rust-mirror on a machine reaching Nexus | Files uploaded into a fresh Raw prefix/repository |
| Activate/use a generation | Nexus operator, then Rustup clients | Operator changes routing or client URLs; Rustup changes installed tools |

`sync` does not publish. `publish` does not activate a group, install Rust, or change
clients. No command starts a scheduler. The unreleased `prune` command can explicitly remove
whole local generations; [retention policy and preview](RETENTION.md) explain its limits. It also does not
run an HTTP server: Nexus or another static host serves the completed tree.

```text
Configuration --sync--> raw generation --rustup--> complete generation
                                                       |
                                                    publish
                                                       v
                                              fresh Nexus Raw storage
                                                       |
                                          explicit operator activation
                                                       v
                                               Rustup client downloads
```

The optional detailed preparation route is `plan` -> `fetch` -> `assemble` instead
of `sync`. Both routes can feed the same `rustup` finishing command.

## Words used in these guides

| Term | Plain meaning | Example |
| --- | --- | --- |
| Tool version | Version of rust-mirror itself | `rust-mirror --version`: 0.1.1 |
| Artifact | One downloaded metadata/archive/installer file | A `.tar.xz` compiler archive |
| Channel | Release stream selected by name | `stable`, `beta`, `nightly` |
| Toolchain | One Rust compiler release and its matching tools/libraries | Rust 1.90.0 |
| Rustup version | Installer/manager version, separate from the compiler | Rustup 1.29.3 |
| Host | OS/CPU where the downloaded compiler runs | `x86_64-pc-windows-msvc` |
| Compilation target | Environment for programs that compiler produces | `wasm32-unknown-unknown` |
| Component | Optional part of a toolchain | Clippy, rustfmt, Rust sources |
| Profile | Upstream's predefined component set | `minimal` or `default` |
| Selection | Your named toolchain/hosts/profile/components/targets choice | `windows-team` |
| Cohort | All selections resolved and frozen together for one sync run | All entries in `mirror.toml` |
| Sidecar | Adjacent metadata for a file | `.sha256` checksum or `.asc` signature |
| Plan | Saved metadata and expected artifact paths/hashes before transfer | `plans/windows/` |
| Snapshot | Verified selected files with an inventory | `snapshot/mirror/` |
| Generation | Immutable directory combining retained snapshots | `complete-002/mirror/` |
| Raw generation | Verified toolchains before the Rustup finishing step | `run-002/mirror/` |
| Inventory | Generated file list, sizes and SHA-256 values | `snapshot.toml` |
| Promotion | Expose an original dated channel as a moving channel | Make 2025-09-18 the mirror's `stable` |
| Raw hosted repository | Nexus storage accepting uploaded ordinary files | `rust-generation-002` |
| Raw group | Read URL routing to a complete hosted generation | `rust-active` |
| Prefix | Path below a repository URL used as the mirror root | `/repository/rust/generation-002` |
| Activation | Make a published generation available at the clients' chosen URL | Change a Nexus Raw group's member |
| Retention | Keep old files so pinned clients still work | Keep 1.89.0 while adding 1.90.0 |

A generation is files, not a virtual machine, server, compiler installation or
container image. A selection's artifact closure means all files required by its
chosen components/hosts/formats. Frozen means the plan records those identities
and will not silently refresh them during replay. Immutable means completed files are not edited in place: create
the next generation to change the mirror.

## Prerequisites and installation

Build with Rust/Cargo 1.90+ and the native linker/C build tools needed by the
platform and dependencies. You need verified HTTPS to `static.rust-lang.org`, a
writable trusted local directory and sufficient disk. A selection can download
hundreds of megabytes; all advertised formats cost more than one client install.

Use a new clone of the current repository, not the separately preserved old
prototype. Skip clone/cd if already in the current checkout:

```powershell
$ErrorActionPreference = 'Stop'
git clone https://github.com/rustcommons/rust-mirror.git
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
cd rust-mirror
cargo build --locked
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
$Mirror = (Resolve-Path .\target\debug\rust-mirror.exe).Path
& $Mirror --version
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Mirror --help
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
```

On Linux/macOS use `./target/debug/rust-mirror`. Below, `rust-mirror` means that
built binary or the installed command on PATH. In PowerShell without a PATH entry,
replace `rust-mirror` with `& $Mirror`. On Linux/macOS, save an absolute path
before changing working folders (for example `Mirror="$PWD/target/debug/rust-mirror"`)
and invoke it as `"$Mirror" sync ...` when the command is not on PATH.

The native CLI needs no Python. Python and Docker/Podman are used by optional
QA/reference scripts, not ordinary preparation/publication. Provision rust-mirror
on the publishing machine before disconnecting, or supply its own build dependencies
through your approved offline process. It does not mirror the Cargo dependencies
needed to build itself. `cargo install` installs the executable, not this manual;
keep a source-checkout/documentation copy with your offline operational materials.
Selecting other compiler hosts also does not cross-build
the rust-mirror executable: that binary must match its own execution machine.

Clients still need
OS prerequisites: Microsoft build tools for Windows MSVC, developer tools on macOS,
and a usable native linker/libraries on Linux. This mirror does not supply SDKs.

## Choose versions, platforms and tooling

Run `rust-mirror init --config mirror.toml` to write editable Linux/Windows/macOS
examples. It does not infer what you need or download anything. Remove unwanted
selections **before** sync: every selection left in the file is processed.

| Host choice | Put this in `hosts` | Native lifecycle tested for 0.1.0 |
| --- | --- | --- |
| Linux, Intel/AMD 64-bit, glibc | `x86_64-unknown-linux-gnu` | Yes |
| Windows, Intel/AMD 64-bit, MSVC | `x86_64-pc-windows-msvc` | Yes |
| macOS, Apple Silicon | `aarch64-apple-darwin` | Yes |
| macOS, Intel | `x86_64-apple-darwin` | Yes |
| Other upstream-advertised hosts | Discover using `list` | Availability is not native qualification |

The machine running rust-mirror need not match these hosts: Windows can download
Linux/macOS archives without executing them. Hosts in one selection share its
profile/components/targets. Use separate selections for different needs.
`x86_64` is Intel/AMD 64-bit; `aarch64` is ARM64. The rest identifies OS/ABI: Linux
GNU/glibc and musl are different choices, as are Windows MSVC and GNU/MinGW. Do
not substitute them merely because the CPU matches. An available metadata flag
is weaker evidence than installing/running on that actual platform.

```sh
rust-mirror list --toolchain 1.90.0
rust-mirror list --toolchain 1.90.0 --host x86_64-pc-windows-msvc
```

`list` returns TOML with release-specific availability, supported profiles, aliases
and targets. It inspects one supplied release/channel, not a historical-version
browser. It reads metadata, not payload archives. An unavailable explicit
component fails the selection; the planner does not search older nightly dates.

- `1.90.0` selects an exact compiler release.
- `stable`, `beta`, `nightly` resolve when a new plan/sync run is created.
- `stable-2025-09-18` or `nightly-YYYY-MM-DD` selects a specific channel/date.
- Config/import selectors must be full numeric versions or supported channels/dates.
  Partial `1.90`, versioned beta selectors and custom/local toolchains are refused.
  Finished mirrors supply ordinary Rustup clients with generated minor aliases
  such as `1.90`; that does not expand the config/import grammar.
- `minimal` and `default` component lists come from the selected release's manifest.
  The latter normally includes standard tools and documentation.
- `complete` is refused because upstream may advertise unavailable components.
  Choose `default` plus explicit available components instead.
- Cargo/Clippy/etc. stay paired with their compiler release. Arbitrary independent
  component-version mixing is not offered.
- `rust-src` supplies source code; a compilation target supplies its standard
  library, not its linker, SDK or execution environment.

To import a project's declared needs:

```sh
rust-mirror import --toolchain-file ./rust-toolchain.toml --name project --hosts x86_64-pc-windows-msvc --rustup-version 1.28.2 --output project-mirror.toml
```

Import creates a new config, requires explicit hosts/Rustup version, does not search
parent folders and does not contact upstream. Review it before downloading. See
[selection/import details](SELECTIONS.md).

## Worked example: first generation

The historical versions below match the actual proof so updates are visible. They
are examples, not a recommendation to freeze your organization on those versions.
Choose supported versions appropriate to your environment.

Use a new private working directory whose parent exists. Save `mirror-001.toml`:

```toml
schema = 1
rustup_version = "1.28.2"
max_download_bytes = 10737418240

[[selections]]
name = "windows-team"
toolchain = "stable-2025-08-07"
hosts = ["x86_64-pc-windows-msvc"]
profile = "minimal"
```

To choose the same selection for all four tested hosts, replace its `hosts` with:

```toml
hosts = ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "x86_64-apple-darwin", "aarch64-apple-darwin"]
```

```sh
rust-mirror sync --config mirror-001.toml --output ./run-001
rust-mirror rustup --snapshot ./run-001/mirror --version 1.28.2 --promote stable-2025-08-07 --output ./complete-001
```

`sync` resolves every selection, freezes metadata, fetches/verifies files and
assembles `run-001/mirror`. `rustup` copies that verified input into a new directory,
adds bootstrap/self-update endpoints and makes the original dated manifest the
`stable` root. It prints `complete-001/mirror` on success.

`toolchain` selects the compiler. `rustup --version` selects the advertised manager.
This generation supplies compiler 1.89.0 and manager 1.28.2, not rust-mirror 1.28.2.

Publish **`complete-001/mirror`**, not its containing directory or private `.sync`
state. Keep the raw `run-001/mirror` for later `sync --previous`. The generated
catalog/aliases are mirror policy; original Rust manifests/signatures stay unchanged.
The generated catalog is not an original signed upstream document.

## Files on disk

```text
working-directory/
  mirror-001.toml              your selections
  run-001/
    .sync.lock                persistent file; OS lock determines ownership
    .sync/                    private plans, downloads, snapshots, report
    mirror/                   verified raw generation
  complete-001/
    mirror/                   transfer/publish this tree
      snapshot.toml           file inventory, not a signature
      dist/                   compiler metadata and selected archives
      rustup/
        release-stable.toml   chosen manager version
        rustup-init.sh        original Unix installer
        archive/VERSION/HOST/ pinned installers/checksums
        dist/HOST/            current installers/checksums
        toolchain-aliases.toml numeric lookup policy
```

`fetch` also has private `.download` state; never host it. Configuration schema 1,
legacy inventory schema 1, selected inventory schema 2 and collection inventory
schema 3 are different formats, not compiler versions or security levels.

## Nexus publication and activation

Use a Raw **hosted** repository, not a proxy expected to fill missing data later.
Provision through normal Nexus administration. Use a fresh repository/prefix,
disallow redeployment, separate upload and client-read access, and protect retained
generations from cleanup. The tool does not provision these settings.

The tested OSS profile uses the following concepts; confirm the equivalent
settings/privilege names in your deployed version rather than treating this as a
production bootstrap script:

| Setting/role | Required purpose | Tested-profile detail |
| --- | --- | --- |
| Storage | Hold complete ordinary files | Raw hosted repository and sufficient blob-store capacity |
| Write policy | Prevent replacing existing files | `storage.writePolicy = ALLOW_ONCE` (no redeploy) |
| Publisher role | Verify/read/upload the new generation only | Repository-scoped `browse`, `read`, `add`, `edit`; no delete/admin rights |
| Client role | Download only | `browse`/`read` on the active group and retained member repositories, as policy permits |
| Group administrator | Explicitly activate a complete member | Separate operator privilege, not the publishing account |
| HTTPS frontend | Preserve verified byte paths | Trusted hostname/chain, no login redirects, upload limits sufficient for chosen archives |
| Cleanup policy | Keep pinned historical releases available | Exclude retained generation data from unwanted deletion |

The tested publisher required Nexus add/edit privileges for the PUT route; that
does not authorize our workflow to overwrite. Server no-redeploy and fresh-prefix
checks remain necessary. MIME handling is addressed below and is never silently
changed by the tool.

For a fixed client URL, create hosted repository `rust-generation-001` and Raw group
`rust-active`. Initially its member is that complete hosted repository. A group
routes requests; it does not construct a coherent Rust release from partial members.
Do not expose it to clients until the first generation is fully published/verified.

Publish at the hosted repository's **root** so paths match the group root. Replace
the example hostname and CA file with an authorized endpoint/trusted bundle. This
command writes files:

```sh
rust-mirror publish --source ./complete-001/mirror --base-url https://nexus.example.local/repository/rust-generation-001 --ca-file /path/to/company-ca.pem
```

Omit `--ca-file` for default publisher trust. A private bundle configures this
connection only; it does not install a CA or configure Rustup trust. Publication
validates local files, checks every destination is absent, writes the inventory
last and GET-verifies all bytes. The inventory marker is administrative metadata,
not a readiness gate that Rustup itself waits for. A failed upload may leave an
incomplete prefix:
never activate it. HEAD/PUT is not a multi-file transaction.

Both `NEXUS_USERNAME` and `NEXUS_PASSWORD` provide Basic publishing credentials.
Neither means anonymous publication only if the server permits it. Prefer a secret
manager. In a dedicated PowerShell shell you can prompt without echoing a password:

```powershell
$env:NEXUS_USERNAME = Read-Host 'Nexus publisher username'
$password = Read-Host 'Nexus publisher password' -AsSecureString
try {
    $env:NEXUS_PASSWORD = [System.Net.NetworkCredential]::new('', $password).Password
    & $Mirror publish --source .\complete-001\mirror --base-url https://nexus.example.local/repository/rust-generation-001 --ca-file C:\certs\company-ca.pem
    if ($LASTEXITCODE -ne 0) { throw 'Publication failed; do not activate.' }
} finally {
    Remove-Item Env:NEXUS_USERNAME -ErrorAction SilentlyContinue
    Remove-Item Env:NEXUS_PASSWORD -ErrorAction SilentlyContinue
    $password = $null
}
```

The environment temporarily contains plaintext credentials; this is not a vault.
Do not enable command tracing or print the environment. Never put credentials in
URLs. Publisher credentials do not configure clients: the tested Rustup route is
anonymous read-only GET. Private client authentication/SSO needs your own tested policy.

The OSS fixture needed a dedicated MIME-policy choice for unchanged armored
signatures. The publisher never alters policy, rewrites signatures or accepts
license agreements. See [hosting/trust](NEXUS-RAW.md) and [server limits](NEXUS-INTEGRATION.md).
After publication and GET verification, the operator can activate the group.
Switches are not atomic across in-flight requests: investigate/retry checksum
failures, never disable verification.

## Bootstrap a Windows client

Use a fresh test account/disposable machine with the required SDK/build tools.
Configure approved OS trust for your internal CA first. This example uses a private
prefix and `--no-modify-path`, not an existing developer's toolchain home.

```powershell
$ErrorActionPreference = 'Stop'
$Base = 'https://nexus.example.local/repository/rust-active'
$HostTriple = 'x86_64-pc-windows-msvc'
$Bootstrap = "$Base/rustup/archive/1.28.2/$HostTriple/rustup-init.exe"
New-Item -ItemType Directory -Path .\client-demo -ErrorAction Stop | Out-Null
$Client = (Resolve-Path .\client-demo).Path
Invoke-WebRequest $Bootstrap -OutFile "$Client/rustup-init.exe"
Invoke-WebRequest "$Bootstrap.sha256" -OutFile "$Client/rustup-init.exe.sha256"
$Expected = ((Get-Content "$Client/rustup-init.exe.sha256" -Raw).Trim() -split '\s+')[0]
if ($Expected -notmatch '^[0-9a-fA-F]{64}$') { throw 'Malformed checksum.' }
if ((Get-FileHash "$Client/rustup-init.exe" -Algorithm SHA256).Hash -ne $Expected) { throw 'Bootstrap checksum mismatch.' }
$env:CARGO_HOME = Join-Path $Client 'cargo'
$env:RUSTUP_HOME = Join-Path $Client 'rustup'
$env:RUSTUP_DIST_SERVER = $Base
$env:RUSTUP_UPDATE_ROOT = "$Base/rustup"
& "$Client/rustup-init.exe" -y --no-modify-path --default-host $HostTriple --default-toolchain none
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
$Rustup = Join-Path $env:CARGO_HOME 'bin/rustup.exe'
& $Rustup set auto-self-update disable
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup toolchain install stable --profile minimal --no-self-update
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup run stable rustc --version
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup toolchain install stable-2025-08-07 --profile minimal --no-self-update
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
```

The PowerShell snippets check `$LASTEXITCODE` explicitly because
`$ErrorActionPreference` alone does not replace native-program exit checks across
PowerShell versions. Stop on every failed download/install;
do not execute an unverified bootstrap. The example expects Rust 1.89.0. If Rustup
detects another installation on an existing account, use a clean test account
rather than installing over it. Close this dedicated shell afterward so its
environment does not redirect normal developer commands.

HTTPS authenticates the server; SHA-256 checks bytes. A checksum downloaded from
the same server is not independent signature verification. Also validate transferred
inventories through your trusted administrative process.

## Bootstrap Linux or macOS clients

Choose the matching host and supply native SDK/linker and TLS trust prerequisites.
Use a fresh account/private prefix. The following is Bash, not PowerShell:

```bash
set -e
Base=https://nexus.example.local/repository/rust-active
Host=x86_64-unknown-linux-gnu  # macOS: aarch64-apple-darwin or x86_64-apple-darwin
mkdir client-demo
Client="$PWD/client-demo"
Bootstrap="$Base/rustup/archive/1.28.2/$Host/rustup-init"
curl --fail --show-error "$Bootstrap" --output "$Client/rustup-init"
curl --fail --show-error "$Bootstrap.sha256" --output "$Client/rustup-init.sha256"
expected=$(awk 'NR==1 {print $1}' "$Client/rustup-init.sha256")
if command -v sha256sum >/dev/null; then
    actual=$(sha256sum "$Client/rustup-init" | awk '{print $1}')
else
    actual=$(shasum -a 256 "$Client/rustup-init" | awk '{print $1}')
fi
[ "$expected" = "$actual" ] || { echo 'Bootstrap checksum mismatch' >&2; exit 1; }
chmod +x "$Client/rustup-init"
export CARGO_HOME="$Client/cargo" RUSTUP_HOME="$Client/rustup"
export RUSTUP_DIST_SERVER="$Base" RUSTUP_UPDATE_ROOT="$Base/rustup"
"$Client/rustup-init" -y --no-modify-path --default-host "$Host" --default-toolchain none
Rustup="$CARGO_HOME/bin/rustup"
"$Rustup" set auto-self-update disable
"$Rustup" toolchain install stable --profile minimal --no-self-update
"$Rustup" run stable rustc --version
"$Rustup" toolchain install stable-2025-08-07 --profile minimal --no-self-update
```

Use a dedicated Bash script/shell: `set -e` stops this sequence on errors. `curl`
and Rustup have separate TLS backends; a CA option for one is not portable trust
configuration for the other. In particular, `SSL_CERT_FILE` alone is not a portable
replacement for approved Windows/macOS OS trust.

The original Unix installer is mirrored at `/rustup/rustup-init.sh`; it obtains
current installers using `RUSTUP_UPDATE_ROOT`. The pinned binary route above makes
the first manager version explicit and allows checking its hash before execution.
rust-mirror does not execute a downloaded installer for you.

## Prepare the next generation

On the connected machine copy `mirror-001.toml` to `mirror-002.toml` and use the
selection below. If you chose multiple hosts earlier, carry that intended host
array into the new configuration too. Retaining an old host does not automatically
select its newer compiler release:

```toml
schema = 1
rustup_version = "1.28.2"
max_download_bytes = 10737418240

[[selections]]
name = "windows-team"
toolchain = "stable-2025-09-18"
hosts = ["x86_64-pc-windows-msvc"]
profile = "default"
components = ["rust-src"]
targets = ["wasm32-unknown-unknown"]
```

```sh
rust-mirror sync --config mirror-002.toml --previous ./run-001/mirror --output ./run-002
rust-mirror rustup --snapshot ./run-002/mirror --version 1.29.3 --promote stable-2025-09-18 --output ./complete-002
rust-mirror publish --source ./complete-002/mirror --base-url https://nexus.example.local/repository/rust-generation-002 --ca-file /path/to/company-ca.pem
```

The operator must first provision fresh hosted repository `rust-generation-002`.
Supply publisher credentials again in its authorized shell; the earlier example
cleared them. This selects compiler 1.90.0 and then manager 1.29.3. The previous
input stays unchanged and its old files remain in the new complete generation.

Use **raw `run-001/mirror`** as previous in this example. Its top-level manager pin
matches `mirror-002.toml` (1.28.2). Sync/assembly refuse incompatible bootstrap pins;
do not substitute a finished tree with a different top-level Rustup version.
Combine raw snapshots first, then finish once to change the manager without losing
old archived installers. `--previous` is a completed local mirror, not a run root,
remote URL or shared global cache; it is fully rehashed.

After successful publication/GET verification, the operator explicitly switches
`rust-active` to the complete second repository. Clients keep the same URLs. If
not using a group/gateway, change both client URLs to the new prefix instead.
The tool performs neither routing action automatically. For a new client starting
directly from this second generation, use the chosen 1.29.3 bootstrap archive
instead of the first-generation 1.28.2 URL.

## Update that same existing client

Keep the earlier client's **same** `CARGO_HOME`/`RUSTUP_HOME`, rather than creating
a new home and calling that an update. After activation, on Windows:

```powershell
& $Rustup update stable --no-self-update
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup run stable rustc --version
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup run stable-2025-08-07 rustc --version
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup component add clippy rustfmt rust-src --toolchain stable
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup target add wasm32-unknown-unknown --toolchain stable
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup self update
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup --version
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup update stable --no-self-update
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
& $Rustup self update
if ($LASTEXITCODE -ne 0) { throw 'Native command failed; stop here.' }
```

In Bash replace `& $Rustup` with `"$Rustup"`. Stable should report 1.90.0, the pinned
dated toolchain still 1.89.0, and the manager 1.29.3. The unchanged repeat should
not fetch compiler archives. Self-update selects the mirrored manager catalog,
not the latest manager on the public internet. Only enable automatic manager
updates after those endpoints and the chosen client version have been verified.

Normal Rustup component/target management works for the files actually selected:

```sh
rustup component remove clippy --toolchain stable
rustup component add clippy --toolchain stable
rustup target remove wasm32-unknown-unknown --toolchain stable
rustup target add wasm32-unknown-unknown --toolchain stable
rustup toolchain install 1.90.0 --profile default --no-self-update
rustup doc --path --toolchain 1.90.0
rustup toolchain install 1.90 --profile minimal --no-self-update
```

Here `rustup` means that client's command; in the private-prefix demo use
`& $Rustup` or `"$Rustup"`, not a different installation from PATH. `doc --path`
prints the installed HTML documentation location without opening a browser.
The minor alias `1.90` means the highest selected stable patch in this generation,
not a newer unmirrored public release. Unselected artifacts are not available
merely because the unchanged original manifest lists them.

For an offline compiler demo, create a dependency-free project and run
`rustup run stable cargo build --offline`. Crates, Git dependencies and native
build tools are separate: 0.1.0 does not mirror the crates.io registry/index.

## Resume, restart and alternative preparation

| Situation | Action | Meaning |
| --- | --- | --- |
| Interrupted sync | Same config/previous/output with `--resume` | Continue the frozen cohort, not today's channel |
| Fresh resolution at an existing sync path | Explicit `--force` | Discard managed old progress/mirror, without backup, then restart |
| New generation with retained history | Fresh output plus `--previous` | Preserve old data and reuse verified bytes |
| Interrupted fetch | Same plan/output with `--resume` | Continue verified files/ranges; fetch has no force |
| Failed plan/finishing/assembly | New output path | Those commands have no resume/force |
| Failed upload | Investigate, then fresh remote prefix | No remote resume/overwrite/force or automatic repair |
| Already present in complete previous data | Sync with fresh output plus previous | May print previous path with `unchanged` and never create NEW |

Force destroys managed old data **before** fresh resolution can succeed. No backup
or rollback is made. It resets local managed output, not old Nexus repositories.
Avoid it on live served output. Active writers, unrelated
paths, links and previous/output overlap are refused. Do not delete lock files to
bypass an OS lock. See [resume recovery](RESUMING.md) and [force/report rules](SYNC.md).

`sync --previous` still checks current upstream metadata, even if no payloads are
needed. Resume uses saved metadata, but missing payload bytes still need source
access. The finishing CLI also reads upstream script/checksums. Complete preparation
and finishing before transferring the verified generation into a disconnected
network. A frozen plan alone is not an offline payload bundle, and the CLI has no
loose-payload cache import option. `assemble` works from completed local inputs;
`publish` contacts the chosen Nexus endpoint rather than upstream.

For explicit per-selection review instead of sync:

```sh
rust-mirror plan --config mirror-002.toml --selection windows-team --output ./plan-windows
rust-mirror fetch --plan ./plan-windows --output ./snapshot-windows
rust-mirror assemble --snapshot ./snapshot-windows/mirror --previous ./run-001/mirror --output ./combined
rust-mirror rustup --snapshot ./combined/mirror --version 1.29.3 --promote stable-2025-09-18 --output ./complete-reviewed
```

Repeat `--snapshot` to combine independently completed selections. A plan freezes
one selection/release; fetch never refreshes metadata. Assembly accepts schema 2/3
inputs, keeps history and refuses immutable conflicts/ambiguous incoming roots.
It does not choose a winner or upgrade the manager. See [planning](SELECTIONS.md)
and [assembly/conflicts](MULTI-VERSION.md).

## Results and troubleshooting

All CLI commands return 0 for success and 1 for errors. Unchanged sync is success.
Paths/reports use stdout; progress/errors use stderr. Check exit status, not just
whether a directory exists: incomplete working state can remain after failure.

| Symptom | Meaning | Next step |
| --- | --- | --- |
| Output exists | No implicit overwrite | Use documented resume/force or a new path |
| Resume identity/config/plan mismatch | Frozen run changed | Restore identical inputs or start fresh; force discards managed old data |
| Lock/writer refusal | Another process owns output | Let it finish/stop normally; do not bypass the lock |
| Unavailable component/host or planning HTTP 404 | Not supplied for that release/date | Inspect `list`, correct the selection or choose an available date |
| Checksum/size mismatch | Not the planned bytes | Retain diagnostics, check source/transfer; never disable verification |
| Budget exceeded | Configured transfer/copy bound reached | Reduce selections or deliberately raise it; also allow staging disk space |
| Destination is not absent | A remote path exists, even if identical | Use a fresh prefix; publisher has no force/resume |
| HTTP 401/403 | Authentication/scoped permission problem | Check both credential variables and repository rights; client access is separate |
| TLS/transport failure | CA/hostname/network/proxy/endpoint issue | Check URL and approved trust; no TLS bypass or redirect support |
| Rustup requests a missing artifact | Not selected, or wrong active generation | Mirror it, publish and activate a new complete generation |
| Stable missing but numeric version works | No stable root selected/promoted | Use dated stable plus promotion, or undated stable selection |
| Unchanged sync but NEW path absent | Previous mirror is the result | Use stdout's actual path; NEW was intentionally not created |
| Native build/link fails after install | SDK/linker/native dependency issue | Supply OS prerequisites; Rust archives are not an SDK mirror |

Publish first, verify second, activate third. Serving an old generation restores
server bytes, **not installed client versions**. Do not promise an automatic
Rustup downgrade: keep pinned toolchains or plan a supported reinstall/rollback.

## Limits, demos and further reading

- Coverage is selected modern distributions, not every historical/custom toolchain,
  unavailable nightly component or every Rustup selector spelling.
- Trust is HTTPS/upstream hashes plus trusted administrative inventories. Original
  OpenPGP files are retained, not independently verified.
- Execution proof covers four hosts and the exact recorded versions. Other advertised
  architectures/musl hosts remain unqualified.
- Actual server proof used isolated legacy OSS 3.76.1-01, not a production deployment
  recommendation. Your Nexus version, CA/auth/proxy/MIME/cleanup policy and current
  Community Edition license onboarding need local assessment.
- No globally atomic activation, hostile-filesystem/NFS/power-loss guarantee,
  automatic retries, shared global cache or resumable uploads is claimed.
- Cargo registry mirroring, scheduling, Nexus pruning, GUI and SDK provisioning are outside
  0.1.0. `rust-mirror-gui` is planned, not an implemented interface.

For a visible small demo without distribution downloads, run
`./scripts/demo-sync.ps1` from a checkout. It uses synthetic archives and Rust tests;
it is not a real toolchain installation. Actual outcomes are in the
[native/Nexus qualification](evals/2026-11-11-rustup-lifecycle.md). Privileged QA
scripts are disposable-test tools, not production Nexus or workstation setup.

- [Every command/option, environment variable, report and library entry point](CLI-REFERENCE.md)
- [Selection/import/planning](SELECTIONS.md)
- [Sync/counters/force](SYNC.md), [resume](RESUMING.md), [assembly](MULTI-VERSION.md)
- [Rustup catalogs, aliases, promotion and groups](RUSTUP.md)
- [Nexus hosting/trust](NEXUS-RAW.md), [QA/server boundaries](NEXUS-INTEGRATION.md)
- [Release checklist](RELEASING.md), [0.1.0 notes](releases/v0.1.0.md), [requirements](REQUIREMENTS.md)

## Local retention (unreleased)

After validating and activating a new generation, use the separate
[retention guide](RETENTION.md) to declare offline-client pins and stable/nightly/
rollback protection. Preview exact deletion paths/sizes before choosing `--apply`.
List all candidate `mirror` roots newest first, active/current first. Nothing prunes
automatically; published 0.1.x binaries have no pruning command. This feature deletes
whole local roots only, leaves retained collections intact and does not delete remote
Nexus copies. Pause other users/writers of deletion candidates before apply.

For GUI-style maintenance, `generations --directory DIR` lists local generations
and run states without verifying payloads. Directory-backed pruning additionally
protects the exact next `--previous` and paused-run dependencies, and refuses changes
since preview. Private run cleanup and general progress/cancel integration remain
deferred; see [discovery and directory planning](RETENTION.md#discover-generations-and-sync-runs).

## Machine progress and Stop (0.2.0)

Use `--events jsonl` for the versioned [CLI/GUI stream](EVENTS.md). Add
`--cancel-stdin` only when a parent keeps the input pipe open and will send cancel
or close it to request Stop. Heartbeats continue during blocked work; unchanged
progress means waiting, while missing heartbeats require the GUI watchdog and
possibly a process kill. Cancellation is cooperative, not rollback. Resume initialized
fetch/sync state explicitly; use a new output for interrupted assembly/finishing,
and inspect partial publication/pruning before retry. Use `publish --preflight`
for a no-write check; actual publication repeats it.

## Corporate network setup (0.3.0)

Start with the [network guide](NETWORK.md). Explicitly choose direct, environment
or manual proxy settings; run `check-network` before a large sync. If TLS inspection
is present, inspect the received issuer and choose the company-approved PEM file;
adding a CA remains your explicit trust decision. Credentials go only through
programmatic worker memory or named secret inputs, never saved settings/argv.
Plan can estimate sizes with HEAD; unknown lengths stay unknown and hashes still
verify downloads. Use `releases --cache DIRECTORY --refresh`, then `--offline` for
cached selectors, and check the reported cache age before choosing a release.

## Curated crate registry (0.4.0)

The experimental [crate MVP](CRATES.md) starts in 0.4.0. Start with an organization-approved catalog spanning projects,
explicit hosts/targets, capacity budgets and the eventual immutable HTTPS base URL.
Keep contact/proxy/CA settings explicit. Do not put credential values in the catalog.

Run `rust-mirror crates sync --config crates.toml --output new-crate-run` to save
a preview without archive downloads. Inspect it with
`rust-mirror crates show --plan new-crate-run/plan.json` and use `crates explain`
for inclusion chains. After approving the whole preview, repeat sync with
`--approve-plan ID` using its exact ID. Resume initialized work with `--resume`;
new upstream versions require a new preview. A successor's `--previous MIRROR`
preserves verified older closures instead of silently pruning them.

Publish the completed `new-crate-run/mirror` to the exact planned HTTPS URL, first
using `publish --preflight`, then normal publication. Activate it separately. On
Windows and Unix, use the same [Cargo source-replacement TOML](CRATES.md#cargo-client-setup)
in a project's `.cargo/config.toml`, preserving any existing configuration.
Run `cargo fetch --locked` and `cargo build --locked` while the internal mirror is
reachable; populated caches can subsequently build with `--offline`. Client CA/proxy
settings remain independent. Pruning requires an explicit crate-retention policy,
a preview and `--apply`; remote deletion remains outside this workflow.
