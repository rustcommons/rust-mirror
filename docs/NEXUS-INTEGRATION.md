# Reproducing the Nexus Raw integration proof

This is a **disposable integration test**, not a Nexus installation, upgrade or
production configuration utility. It never targets an externally supplied Nexus URL.
Use the normal [publication guide](NEXUS-RAW.md) for an already provisioned server.

## Tested profile and current boundary

The legacy compatibility target is the official Nexus Repository **OSS 3.76.1-01**
image. This version is [sunset according to Sonatype](https://help.sonatype.com/en/sonatype-nexus-repository-3-versions-status.html).
It is used solely to test the OSS Raw protocol without newer Community Edition
onboarding; this is not a recommendation to deploy it or keep it in production.
An existing production installation's actual version/configuration needs its own test.

Community Edition **3.90.6-01** was also started in isolation. It refused repository
content with HTTP 403 until the operator accepts its EULA. No EULA acceptance API
or bypass is included in our proof or publisher. Its authenticated upload/install
path remains held, not passed. See Sonatype's [Community Edition onboarding](https://help.sonatype.com/en/ce-onboarding.html).
Do not substitute a legacy server for a supported deployment to avoid an agreement.
Use your existing licensed/onboarded instance or make that decision with its owner.

The proof keeps the actual Nexus server and client in a network namespace containing
only loopback and no external routes. No ports are published to the host. It uses a
new dedicated volume, generated credentials, scoped roles, a temporary CA and a
verified HTTPS reverse proxy forwarding to Nexus's loopback HTTP listener. This
validates TLS termination at that proxy; it does not test Nexus-native TLS or your
production proxy/CA/SSO configuration.

## Prerequisites and command

Linux root/namespace permissions, existing Docker or rootful Podman, OpenSSL, Python
3.11+ and an already prepared pinned snapshot are required. The runner does not
install tools or pull an image silently. Budget roughly 2 GB memory for the test
container and space for image, snapshot, isolated clients and temporary data.
Never run it in place of a production instance.

For the exact legacy profile only:

```sh
sudo podman pull docker.io/sonatype/nexus3@sha256:390b8ec7e712c680ed8b449b766695a7c5ebbf41e489317618e63ca199f2cc09
sudo python3 scripts/run_nexus_proof.py \
  --runtime podman \
  --image docker.io/sonatype/nexus3@sha256:390b8ec7e712c680ed8b449b766695a7c5ebbf41e489317618e63ca199f2cc09 \
  --allow-raw-mime-types \
  --snapshot /path/to/completed-snapshot/mirror \
  --evidence /tmp/rust-mirror-nexus-proof-new
```

Docker can be selected with `--runtime docker` and the same explicitly pulled image.
The evidence directory must not exist. `--image` is required and is recorded with
its resolved image ID/digests. The runner creates uniquely named/labeled resources,
validates their ownership and network mode, joins only that container's network
namespace, then removes its container/volume and the initial password file on exit.
It retains the image cache and proof logs; a forced process/machine interruption
can prevent cleanup, so inspect the recorded resource IDs before manual cleanup.
It never performs a global container stop, prune, image update or shared-data repair.

The manual `Legacy Nexus OSS compatibility proof` workflow repeats this profile.
It downloads the pinned toolchain snapshot on the connected runner before starting
the network-disabled proof. It does not establish Community Edition qualification.

## Meaning of the test-only MIME opt-in

The proof first creates a Raw hosted repository with strict MIME validation enabled
and redeployment disabled (`ALLOW_ONCE`). The unchanged Rust channel `.asc` file is
the original upstream armored signature (not independently OpenPGP-verified here), but the tested Nexus MIME detector identifies
it as text/plain and expects application/pgp-signature based on its extension.
This results in HTTP 400; changing a Content-Type request header does not repair the
content/name comparison. We do not alter the signature, convert it or skip it.

Only when `--allow-raw-mime-types` is explicitly supplied may the test change its
**own new disposable Raw repository** to `strictContentTypeValidation=false`. It
then retries the same bytes and verifies their GET identity. Redeployment refusal,
scoped permissions, trusted TLS and mandatory local/remote SHA-256 checks stay in
force. Without the opt-in, this server profile remains held at the MIME-policy gate.
Sonatype [documents this repository option](https://help.sonatype.com/en/raw-repositories.html)
for Raw MIME-type compatibility problems.

The production publisher does **not** configure a server or switch this setting.
Your Nexus owner must choose an appropriate dedicated-repository policy or a
version/configuration whose MIME validation accepts the original files. Disabling
MIME validation removes that server-side content/extension check; hash verification
and signatures are separate controls, not a claim that the setting has no tradeoff.
No TLS verification or authentication is disabled by this flag.

## Trust and credentials

The publisher now accepts:

```sh
python3 scripts/publish_raw.py \
  --source ./snapshot/mirror \
  --base-url https://nexus.example.local/repository/rust-toolchains/snapshot-001 \
  --ca-file /path/to/internal-ca-bundle.pem
```

`--ca-file` selects a PEM trust bundle **instead of the default system roots for
this publisher connection**. Include all required trust anchors. It verifies both
the certificate chain and hostname; it is not a skip-verification switch and is
refused on HTTP. Omit it to use system trust. Use the existing environment-based
Basic credentials only on HTTPS. Credentials are never placed in URLs or recorded
in proof summaries.

Rustup is a different client: the tested Rustup 1.28.2 route receives `SSL_CERT_FILE`
and `CURL_CA_BUNDLE` pointing to the temporary public CA certificate. Publisher
credentials are not passed to it. The proof grants anonymous read-only access to
one dedicated repository via a separately created reader identity. It rejects
anonymous writes, wrong passwords and cross-repository access. This does not prove
password-protected Rustup downloads, enterprise SSO or arbitrary authentication modes.

## Evidence and remaining work

See [the dated results](evals/2026-11-09-nexus-integration.md). Failed setup attempts,
Community Edition's EULA gate, the strict-MIME failure and successful paths remain
separate. No production server, package version, registry publication, source tag,
user toolchain home or original prototype is changed by this iteration.

## Selection inventory boundary

The original result above is the schema 1 minimal-toolchain case. The expanded
driver now accepts schemas 2/3, validates their selected records and optionally
exercises an old/new pair through a fixed Raw group URL. Pass `--publisher-cli`
to the runner to test the installed native Rust publisher rather than the Python
reference helper. [Current results](evals/2026-11-11-rustup-lifecycle.md) identify
which exact paths passed; no Community Edition or production-instance pass follows.
See [Rustup lifecycle and explicit activation](RUSTUP.md) and the
[generation guide](MULTI-VERSION.md).

## Native Windows qualification

The [Windows job](../.github/workflows/nexus-windows.yml) runs Nexus OSS 3.76.1-01
natively, using the official win64 ZIP and its bundled JRE, rather than Docker.
The legacy-version/support caveat above still applies. The
[dated Windows record](evals/2026-11-14-windows-nexus.md) identifies exact source,
Actions runs, successful/refused cases and limits; historical Linux-only records
remain historical.

The [runner](../scripts/run_nexus_windows.py) checks both Sonatype's published SHA-256
sidecar and pinned digest before extracting into a new marked directory. It starts
`nexus.exe /run` without installing a service or accepting any agreement. A verified
HTTPS terminator fronts its loopback-only HTTP listener; this tests proxy TLS, not
Jetty-native TLS. The job keeps a random admin password, rotates it, creates scoped
publisher/anonymous-reader roles and uniquely named no-redeploy Raw repositories.
Passwords, Nexus logs/data and PKI private keys are never uploaded.

On a disposable Windows machine with Python 3.12, Git's OpenSSL and the built CLI:

```powershell
python scripts/run_nexus_windows.py --binary target/debug/rust-mirror.exe `
  --root C:\Temp\rust-mirror-owned-nexus-new `
  --evidence C:\Temp\windows-nexus-evidence-new
```

Both directories must be new. Allow space for the roughly 272 MiB download, extracted Nexus,
crate/toolchain mirrors, isolated Cargo builds and the fresh minimal Rustup install.
The Actions job also selects `--with-toolchain`, restricted to disposable GitHub-hosted
Windows runners. After Cargo's CA-file-only checks, this optional stage imports only
the temporary public CA into LocalMachine ROOT for Windows Rustup, then removes
its exact thumbprint and verifies removal. The owned marker permits interruption
cleanup to retry that removal. It adds a separately named test Raw repository with
explicit MIME relaxation for unchanged upstream toolchain signatures; the crate
repository retains strict MIME validation. The production publisher never changes
server policy. A startup/EULA/content gate refuses instead of accepting terms.

The crate case uses real serde/serde_json/anyhow roots and pinned hexyl 0.16.0 with
its real dev-dependencies. Its source archive is separate from mirrored registry
packages. A bounded CONNECT tunnel rate-limits encrypted real upstream traffic for
an actual Windows process kill; upstream TLS remains end to end and verified.
Resume reuses the frozen plan and verifies all completed bytes. The Windows Cargo
client replaces crates.io with the Nexus sparse registry in a fresh CARGO_HOME,
trusts the test CA through CARGO_HTTP_CAINFO, fetches/builds locked, then builds
offline into a second fresh target directory. No user Rustup/Cargo homes are used.

Normal exit and the workflow's always-run cleanup delete only the marked owned
instance/data. A recorded PID must still name that instance's executable before
its process tree can be stopped. Process/machine interruption can prevent cleanup;
the workflow retries owned cleanup and a hosted VM is ultimately discarded. No
Internet/air-gap isolation or arbitrary enterprise Nexus/SSO configuration is claimed.
