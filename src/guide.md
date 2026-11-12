# rust-mirror

Prepare selected Rust toolchains for offline Rustup clients, then publish a finished
generation to a fresh Nexus Raw/static HTTPS prefix. Rust/Cargo 1.90+ is required.
This library does not install client tools, activate routing or mirror Cargo crates.

Use the [user guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/USER-GUIDE.md)
for the full setup/recovery workflow and the
[command reference](https://github.com/rustcommons/rust-mirror/blob/main/docs/CLI-REFERENCE.md)
for configuration, effects and limits.

## Prepare a mirror

Save explicit selections in TOML; [`selection::Config::read`] validates the file.
Use new output directories and retain the actual returned mirror path:

```no_run
use rust_mirror::{selection, HttpsSource};
use std::path::Path;
let config = selection::Config::read(Path::new("mirror.toml"))?;
let run = selection::sync::run(&config, None, Path::new("run-001"), false, &HttpsSource::default())?;
let finished = selection::rustup::prepare(&run.mirror, "1.29.3", &[],
    Path::new("generation-001"), config.max_download_bytes, &HttpsSource::default())?;
println!("{}", finished.display());
# Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
```

## Progress, stop and publication

[`events::Control`] combines a thread-safe [`events::Observer`] and
[`events::CancelToken`]. Controlled `_with_control` operations emit stages, artifact
bytes, hashing and independent heartbeats. [`ErrorKind`] and [`error_kind`] expose
machine-readable failures. Cancellation preserves initialized fetch/sync state for
explicit resume; publication/pruning may already have partial writes/deletions.
See the [versioned protocol and recovery contract](https://github.com/rustcommons/rust-mirror/blob/main/docs/EVENTS.md).

[`publish::preflight`] checks local closure and all remote absences without writes;
[`publish::publish`] repeats checks and verifies uploads. TLS/auth options are
explicit; the library does not read CLI credential environment variables. See
[hosting/trust](https://github.com/rustcommons/rust-mirror/blob/main/docs/NEXUS-RAW.md).

## Corporate networks and release discovery

[`NetworkOptions`] and [`HttpsSource::with_network`] configure explicit proxy/auth,
exclusions, extra CA trust and socket timeouts. Defaults stay direct; extra CAs add
to Mozilla roots. [`network::detect_environment`] is an explicit inspection call;
[`network::inspect_ca_file`] returns display-only certificate metadata.
[`network::check_network_with_control`] diagnoses connection stages without bypassing
TLS. Secrets are memory-only; GUI workers pass [`network::ProxyCredentials`] directly.
See the [network guide](https://github.com/rustcommons/rust-mirror/blob/main/docs/NETWORK.md).

[`selection::measure_sizes`] returns opt-in HEAD estimates, while
[`selection::sync::run_with_options`] enables estimated aggregate progress. Hashes and
actual budgets still decide verification. [`releases::load`] supports fresh discovery
and explicit offline cache reads, reporting cache age. Publishing uses its separate
[`publish::preflight_with_network`] and [`publish::publish_with_network`] APIs.

## Local retention

[`selection::retention::discover`] lists metadata;
[`selection::retention::preview_directory`] protects the exact next-sync anchor and
paused dependencies. [`selection::retention::apply`] requires explicit authority,
revalidates the preview and reports partial outcomes. Only whole completed local
mirror roots are removed. See [retention](https://github.com/rustcommons/rust-mirror/blob/main/docs/RETENTION.md).

Upstream hashes/trusted local storage are the trust model; signatures are preserved
without independent OpenPGP validation. No hostile-filesystem, NFS, globally atomic
activation or arbitrary-platform guarantee is implied. Read the
[requirements and evidence](https://github.com/rustcommons/rust-mirror/blob/main/docs/REQUIREMENTS.md)
and [release notes](https://github.com/rustcommons/rust-mirror/blob/main/docs/releases/v0.2.0.md).
