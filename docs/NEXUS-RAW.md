# Hosting a toolchain snapshot in Nexus Raw

## Supported shape and current evidence

Use a **Nexus Repository 3 Raw (hosted)** repository. It serves ordinary bytes at
stable paths, which is what Rustup's distribution protocol needs. Do not depend on
an online Raw proxy cache to fill missing files inside a disconnected network.
Sonatype documents [HTTP PUT uploads and GET retrieval](https://help.sonatype.com/en/raw-repositories.html#uploading-files-to-hosted-raw-repositories).

The first proof used a local HTTP PUT/HEAD/GET fixture. A subsequent
[actual OSS compatibility proof](NEXUS-INTEGRATION.md) tested Nexus OSS 3.76.1,
scoped publishing credentials, read-only clients, a trusted HTTPS reverse proxy,
no-redeploy and a fresh offline installation. It required an explicit dedicated
Raw repository MIME policy. This old server is a test fixture, not a deployment
recommendation. Community Edition 3.90.6 is held at its operator EULA gate.
Your actual version, certificate/access/cleanup policy and proxy still need an
instance-specific test; no production instance has been contacted.

For named versions/channels, additional components, targets or multiple hosts,
use the [selection workflow](SELECTIONS.md). Its schema 2 inventory is accepted
by the same publisher. The legacy command below remains a schema 1 minimal example.
Only the recorded original schema 1 case has actual Nexus server qualification.
For a shared multi-version tree, [assemble a schema 3 generation](MULTI-VERSION.md)
and publish it through the same helper to a new prefix.

## Publish from the installed CLI

The Rust `publish` command needs no Python runtime. It verifies schema 1/2/3 local
inventories and selected data, checks every remote destination is absent before
any PUT, uploads the inventory last and GET-verifies every remote byte:

```sh
rust-mirror publish --source ./complete-001/mirror --base-url https://nexus.example.local/repository/rust-generation-001 --ca-file /path/to/company-ca.pem
```

Set both `NEXUS_USERNAME` and `NEXUS_PASSWORD` in the invoking process for HTTP
Basic authentication over HTTPS; never put credentials in a URL or command argument.
Read them securely without echo/history and clear them after publication. An
anonymous publication is attempted only when neither variable is set and the
server explicitly permits it. The caller must have read, browse and upload rights
for this new prefix. The library takes credentials explicitly and reads no environment.

For private TLS, pass a PEM CA bundle (at most 4 MiB / 256 certificates). This
configures the publisher's connection, not Rustup's client trust. No verification
bypass, credential-bearing URL, HTTP redirect or automatic proxy discovery is used.
Literal loopback HTTP requires the test-only `--allow-loopback-http`, and can never
carry credentials or a CA option. Each request has a 180-second deadline.

Stdout is a TOML report with `files`, `remote_bytes_verified`, `inventory_sha256`
and `schema`. Success is 0; failures return 1. Credentials are not printed.
The byte count is verified payload bytes, not HTTP/TLS traffic. All destinations
are checked before writes; concurrent writers still require server no-redeploy and
exclusive publication. Interrupted/failed uploads can leave an incomplete prefix:
never activate it, retry writes blindly or treat HEAD/If-None-Match as a transaction.
There is no remote `--force`, upload resume or automatic group/settings change.
The Python helper below remains a reference/QA interface, not a CLI runtime dependency.
For fixed client URLs and explicit activation, see [Raw groups](RUSTUP.md#keep-one-client-url-with-a-nexus-raw-group).

## Prepare on the connected machine

Requirements: Rust/Cargo 1.90 or newer, HTTPS access to static.rust-lang.org and a
new output directory whose parent already exists. The prototype's original files
remain preserved separately; this is the new experimental implementation.

```sh
cargo build --locked
cargo run --locked -- fetch-toolchain \
  --release 1.90.0 \
  --host x86_64-unknown-linux-gnu \
  --rustup-version 1.28.2 \
  --output ./snapshot-1.90.0
```

Output is `snapshot-1.90.0/mirror/`. The output must not already exist. A failed
attempt can leave a reserved output directory; select a new output for a retry.
It never produces a completed `mirror/` tree on a verification error. This is not
resumable or a general shared-cache implementation. Use a trusted private parent;
concurrent mutation by other local users is outside this first proof.

The tree contains the untouched pinned channel manifest, its original `.sha256`
and `.asc`, the host's selected minimal-profile component archives, and the pinned
Rustup bootstrap plus its original checksum. `snapshot.toml` lists every mirrored
file, its size and SHA-256. Transfer the entire tree unchanged to your offline
publishing machine. No archive extraction is performed by `rust-mirror`.

SHA-256 is checked against upstream metadata fetched over HTTPS. OpenPGP signatures
are retained but not independently verified. The inventory detects byte changes;
it is not a signed transfer authorization or an independent trust root.

## Upload to a hosted Raw repository

Provision the repository through your normal administration workflow. Choose a
new immutable prefix per snapshot, for example:

```text
https://nexus.example.local/repository/rust-toolchains/rust-1.90.0-linux-001/
```

Requirements for the publisher account: HEAD/GET and upload permissions for that
prefix. Configure the repository to disallow redeployment and protect completed
snapshots from cleanup. Use trusted HTTPS and an internal CA trusted by Python
and Rustup; do not disable certificate verification. Keep write permissions separate
from client read access where possible.

Python 3.11+ is needed for the publication helper. Supply `NEXUS_USERNAME` and
`NEXUS_PASSWORD` through your secret manager/environment if authentication is
required. Do not put passwords into command arguments, URLs, Git or documentation.

```sh
python3 scripts/publish_raw.py \
  --source ./snapshot-1.90.0/mirror \
  --base-url https://nexus.example.local/repository/rust-toolchains/rust-1.90.0-linux-001
```

For an internal CA, add `--ca-file /path/to/trusted-ca-bundle.pem`. This selects
a PEM trust bundle instead of default roots for the publisher connection; chain
and hostname checks remain enabled. See [trust details](NEXUS-INTEGRATION.md#trust-and-credentials).

The helper checks all local inventory entries before network writes, refuses
unexpected files/symlinks, requires destinations to return HEAD 404, sends each
file with PUT, and verifies every uploaded file using GET and SHA-256. It uploads
archives before channel metadata and writes `snapshot.toml` last. It does not
provision a repository, implement arbitrary authentication plugins, proxy the
upstream, prune files or repair an incomplete publication. On failure, retain the
logs and choose a new snapshot prefix after investigating.

The tested OSS server rejected the unchanged armored `.asc` under strict MIME
validation. Its owner must select a suitable dedicated Raw repository policy; the
publisher never changes server settings or rewrites the signature. The exact
[test-only opt-in and tradeoff](NEXUS-INTEGRATION.md#meaning-of-the-test-only-mime-opt-in)
are documented separately.

HTTP PUT is **not** a multi-file transaction. HEAD checks and `If-None-Match: *`
are not a substitute for server-enforced no-redeploy policy and exclusive publishing.
Do not give clients the snapshot URL until publication and GET verification finish.
There is no rolling `stable` alias or automatic switch of existing clients yet.

The relevant layout below is illustrative; archive names come from the upstream
manifest rather than being synthesized:

```text
rust-1.90.0-linux-001/
  snapshot.toml
  dist/channel-rust-1.90.0.toml
  dist/channel-rust-1.90.0.toml.sha256
  dist/channel-rust-1.90.0.toml.asc
  dist/2025-09-18/{cargo,rustc,rust-std}-...tar.xz
  rustup/archive/1.28.2/x86_64-unknown-linux-gnu/rustup-init
  rustup/archive/1.28.2/x86_64-unknown-linux-gnu/rustup-init.sha256
```

## Install on the disconnected Linux machine

The first demonstrated client is x86_64 Linux with its system loader, native linker
and libraries already installed. Mirroring Rust does not supply those operating
system prerequisites or third-party build-script dependencies.

```sh
export RUSTUP_DIST_SERVER=https://nexus.example.local/repository/rust-toolchains/rust-1.90.0-linux-001
export RUSTUP_UPDATE_ROOT="$RUSTUP_DIST_SERVER/rustup"
```

Download the pinned bootstrap from
`$RUSTUP_DIST_SERVER/rustup/archive/1.28.2/x86_64-unknown-linux-gnu/rustup-init` and
verify its SHA-256 using the transferred, independently checked inventory before
executing it. Authenticated browser/curl access may be used for this bootstrap.
The client distribution URL must subsequently be readable by Rustup: the actual
OSS proof covers anonymous read-only GET, not password-protected Rustup downloads. Do not assume the
publisher's HTTP credentials configure Rustup. An internal read-only anonymous
repository or an appropriately configured authenticated gateway needs an explicit
local access policy and an instance-specific test.

For a new account, after verifying the bootstrap:

```sh
chmod +x rustup-init
./rustup-init -y --no-modify-path --default-host x86_64-unknown-linux-gnu --default-toolchain none
"$HOME/.cargo/bin/rustup" set auto-self-update disable
"$HOME/.cargo/bin/rustup" toolchain install 1.90.0 --profile minimal --no-self-update
"$HOME/.cargo/bin/rustup" run 1.90.0 rustc --version
```

Use dedicated `CARGO_HOME` and `RUSTUP_HOME` if evaluating this without affecting
an existing account. `--no-modify-path` leaves shell setup to you. A complete fresh
home example is implemented by [the proof driver](../scripts/prove_offline.py).

The manifest remains byte-identical to upstream. Rustup redirects official archive
URLs through `RUSTUP_DIST_SERVER`, including its repository prefix; no manifest
rewrite is needed. See the [Rustup environment reference](https://rust-lang.github.io/rustup/environment-variables.html)
and [tested Rustup 1.28.2 behavior](https://github.com/rust-lang/rustup/blob/1.28.2/src/dist/manifestation.rs#L137-L160).

## Scope limits

The legacy command/example in this guide is the recorded pinned 1.90.0 minimal
GNU Linux case with Rustup 1.28.2. It selects one preferred format (zstd, xz, gzip).
That historical proof does not qualify every release or client. The newer
[selection workflow](SELECTIONS.md) downloads all selected advertised variants and
has separate component/Wasm evidence; [multi-version generations](MULTI-VERSION.md)
have a separate two-release Linux coexistence proof. Foreign-platform execution,
old-system qualification, self-updates and deployed Nexus policy remain gaps.

The untouched manifest lists components/targets that this subset snapshot does
not contain. Requests for those artifacts will fail; the inventory, not the full
upstream manifest, defines downloaded coverage. Crates.io dependencies are not
mirrored. A dependency-free Cargo offline build is proof of the installed toolchain,
not proof that arbitrary Cargo projects build offline. The GUI is not implemented.

See [the exact proof record](evals/2026-11-09-offline-toolchain.md).
