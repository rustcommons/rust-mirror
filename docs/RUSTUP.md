# Rustup lifecycle mirror

New here? Start with the [complete user guide](USER-GUIDE.md). For all flags,
defaults and outputs see the [command reference](CLI-REFERENCE.md).

This is the implemented distribution workflow for published 0.1.0. The
[prepublication gates](RELEASE-0.1.0-PLAN.md#current-disposition) passed; publication
and official postpublication checks remain pending.
Scheduling is excluded. Rustup remains the installer and manages clients normally.

## Choose platforms, tools and versions

Use named [selections](SELECTIONS.md). Explicit hosts may include
`x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`, `x86_64-apple-darwin`,
`aarch64-apple-darwin`, GNU/musl Linux and other upstream-advertised platforms.
Discovery and planning reject unavailable hosts/components instead of substituting.
Select `minimal` or `default`, additional components, compilation targets, and
numeric/stable/beta/nightly/date selectors. Compiler SDKs and system libraries
are supplied by the client OS, not Rustup or this mirror. In particular, Windows
MSVC still needs Microsoft build tools and macOS needs its platform developer tools.

Run `sync` to resolve/freeze/download/verify/assemble all selections. Interrupted
runs need explicit `--resume`; destructive `--force` discards the old run with no backup.

## Add Rustup bootstrap and self-update endpoints

```sh
rust-mirror rustup --snapshot ./run-001/mirror --version 1.29.3 --output ./complete-001
```

The fresh `complete-001/mirror` retains all verified toolchains and archived
installers. For every recorded host it supplies the chosen pinned Rustup installer
under `rustup/archive/VERSION/HOST/` and `rustup/dist/HOST/`, including checksums.
It copies the original upstream shell installer to `rustup/rustup-init.sh`.
`rustup/release-stable.toml` is a **generated mirror policy catalog** selecting the
requested Rustup version; it is not represented as an original signed upstream
manifest. Rust toolchain manifests/signatures remain byte-identical originals.

All metadata/checksums are frozen before new installer payload transfers. Existing
verified archives can be reused; changed immutable archive identities refuse.
Copy-time hashes, whole installer hashes, byte/file bounds and final inventory
validation precede exposing the completed tree. Only new output is accepted; this
small finishing stage has no resume/reset option. Its budget is the completed
snapshot, plus a private staging copy and the retained external input on disk.
A failure leaves no completed mirror. The default byte budget is 10 GiB; select
`--max-bytes` explicitly if needed. No serving, publication or client activation
happens automatically.

The finishing stage also supplies full numeric and major.minor Rustup client
lookups (for example `rustup toolchain install 1.90`). A minor alias chooses the
highest selected stable patch already in this generation, not an unmirrored public
release. `rustup/toolchain-aliases.toml` records that generated policy. Its manifests
and signatures are original bytes copied to those lookup paths. Mirror selection
configs remain explicit full versions or named/date channels; they do not silently
resolve a partial numeric version during offline project-file import.

## Promote a channel and update existing clients

For reproducible historical promotion, select e.g. `stable-2025-09-18` during sync,
then explicitly make that original dated metadata the stable root in a new generation:

```sh
rust-mirror rustup --snapshot ./run-002/mirror --version 1.29.3 --promote stable-2025-09-18 --output ./complete-002
```

`--promote` accepts one dated selector each for stable, beta or nightly. The verified
snapshot must contain a matching original dated selection and metadata triplet.
There is no guessed version, backwards date search or mixed manifest/checksum/signature.
An undated `stable` selection already supplies its resolved stable root.

Publish each generation with the native `publish` command to a new [Nexus Raw prefix](NEXUS-RAW.md). Keep the old prefix
available so pinned clients retain historical artifacts. Point clients at the new
verified generation (or deliberately switch an operator-managed gateway route):

```sh
export RUSTUP_DIST_SERVER=https://nexus.example/repository/rust/complete-002
export RUSTUP_UPDATE_ROOT="$RUSTUP_DIST_SERVER/rustup"
rustup update stable --no-self-update
rustup component add clippy rustfmt rust-src --toolchain stable
rustup target add wasm32-unknown-unknown --toolchain stable
rustup self update
```

PowerShell uses `$env:RUSTUP_DIST_SERVER = 'https://...'` and
`$env:RUSTUP_UPDATE_ROOT = "$env:RUSTUP_DIST_SERVER/rustup"`.
Client TLS trust is separate from `publish --ca-file`. Rustup uses its selected
backend's platform verifier; set up the organization's CA through its approved OS
policy on Windows/macOS. `SSL_CERT_FILE` is not a portable substitute for that
platform trust setup. The proof's temporary CA installation is CI-only, not a
feature that changes user trust stores. See the pinned
[Rustup download implementation](https://github.com/rust-lang/rustup/blob/1.28.2/src/download/mod.rs).

The same installed CARGO_HOME/RUSTUP_HOME is retained: `stable` advances while a
pinned dated toolchain remains pinned. Repeat updates should download no toolchain
archives when unchanged. Self-update selects the **mirrored Rustup catalog**, not
the latest Rustup on the public internet. Only enable automatic self-updates after
your published generation includes and has verified these endpoints for the host.
Updating a mirror generation alone does not switch client environment settings;
there is no automatic Nexus routing change or multi-file transactional overwrite.

## Keep one client URL with a Nexus Raw group

For a fixed `RUSTUP_DIST_SERVER`, an operator can create one no-redeploy Raw hosted
repository per **complete generation**, publish each at its repository root, and
expose a Raw group containing the active hosted repository. Clients use the group
root and its `/rustup` subdirectory. Publish and verify the new repository completely,
then explicitly switch the group's member to it in Nexus. This keeps client URLs
unchanged without overwriting artifact files or changing credentials on clients.
The mirror CLI/publisher does not silently make this administrative change.

Every new generation must retain old immutable archives and dated manifests that
existing clients need. Do not configure a group that mixes incomplete subsets:
[Nexus returns the first matching member](https://help.sonatype.com/en/raw-repositories.html),
not a semantically merged Rustup release. Keep old hosted repositories available
for rollback and pinned clients; no pruning is performed here. Scope publication
credentials to the new hosted repository and group-administration privileges to
an operator, with clients read-only.

A group switch is not a transaction across requests already in flight. A client
fetching a checksum during activation can need a retry; do not claim globally atomic
client updates. The actual Nexus fixture checks an existing client installing the
old stable release and then advancing stable and Rustup through the **same group
URL** after explicit activation. This supplements the separately recorded
old-prefix/new-prefix client workflow; it does not configure a production server.

## Bootstrap new clients

Use the pinned host `rustup/archive/VERSION/HOST/rustup-init[.exe]` and check its
SHA-256 before execution, or use the original mirrored Unix installer script over
trusted HTTPS with `RUSTUP_UPDATE_ROOT` set to the mirrored `rustup` directory.
Initialize with `--no-modify-path` and `--default-toolchain none`, then install the
selected toolchain/profile/components/targets. Use disposable CARGO_HOME/RUSTUP_HOME
for testing. Never copy the mirror's private `.sync` or `.download` state to clients.

## Permanent qualification

`scripts/prepare_rustup_lifecycle.py` builds real old/new generations for a native
host. `scripts/prove_rustup_lifecycle.py` checks fresh bootstrap, existing-client
updates, pinned history, component/target changes, offline builds, self-update,
repeat operations and missing/corrupt download controls. The dedicated workflow
runs Linux network namespaces, macOS sandboxing and program-scoped Windows firewall
rules. Temporary CA trust changes on macOS/Windows are restricted to disposable
fresh GitHub-hosted VMs. Windows uses the machine Root store and exact-certificate
removal. macOS attempts bounded exact removal; if the headless API times out, the
proof records deferred VM-teardown cleanup and destroys both fixture private keys.
That is not a successful explicit-removal claim. Self-hosted runners and workstations
are refused for native trust setup. Never run the Windows wrapper on a workstation.

The extended Nexus runner accepts selected/collection snapshots and an optional
`--next-snapshot` for the same lifecycle through actual Raw storage with a trusted
TLS proxy, scoped writer and read-only clients. It uses an already-pulled explicit
image in an owned isolated container and never accepts license agreements.
The original legacy proof remains historical. The [current qualification record](evals/2026-11-11-rustup-lifecycle.md#final-prepublication-qualification)
records the four native hosts, schema 2/3 Nexus cases and exact evidence boundaries.

HTTPS/upstream checksum and administrative inventory trust are required. Original
OpenPGP signatures are retained; independent signature verification is not added
by this finishing command. All supported selections are mirrored, not every
historical release or unavailable upstream component. Native results establish only
the host/version/server combinations actually recorded. No Cargo registry mirroring,
GUI, SDK provisioning, scheduling, retention pruning or production Nexus qualification
is implied.
