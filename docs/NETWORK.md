# Corporate-network contract for 0.3.0

Owner-approved scope, 2026-11-12. **Implemented in the unreleased 0.3.0 candidate;
not available in 0.2.0.** This guide defines the library/GUI contract and bounded
validation. The coordinator handles publication after owner confirmation; this
thread does not publish or create the 0.3.0 release tag.

## Network options and secrets

Public `network::{NetworkOptions, ProxyMode, ProxyCredentials, NetworkConfig}` and
`HttpsSource::with_network(&NetworkOptions)` configure every upstream request:
metadata, artifacts, ranges, HEAD estimates, Rustup bootstrap and release discovery.
`NetworkOptions` has `proxy`, optional in-memory `proxy_credentials`, `no_proxy`,
`extra_ca_file`, `connect_timeout` and `read_timeout`. `ProxyMode` is `Direct`,
`Environment` or `Manual(String)`. Default is direct, with 180-second connect/read
timeouts; positive explicit timeouts are required. Network options are supplied
separately from cancellation/progress `events::Control`.

`ProxyCredentials` contains programmatic username/password, has redacted debug output
and is not serializable. The GUI passes secrets to its own worker over stdin and
constructs this value there. Neither library options nor reports persist secrets.
CLI/config use environment variable **names**, never credential values; stdin is
also supported for a proxy password. Reject userinfo in explicit/config proxy URLs.
Never print raw transport errors containing proxy credentials or authorization headers.

Optional schema-1 `[network]` settings use `NetworkConfig`, with `proxy_mode`
(`direct`, `environment`, `manual`), optional `proxy_url`, `proxy_username_env`,
`proxy_password_env`, `no_proxy`, `extra_ca_file`, `connect_timeout_seconds` and
`read_timeout_seconds`. Only variable names may be serialized for credentials.
CLI flags are `--proxy URL`, `--proxy-from-env`, `--no-proxy LIST`,
`--proxy-username-env NAME`, `--proxy-password-env NAME`, `--proxy-password-stdin`,
`--extra-ca-file PEM`, `--connect-timeout SECONDS`, `--read-timeout SECONDS`.
`--network-config FILE` optionally loads the `[network]` section of a validated
mirror config for commands without their own config file. `plan` and `sync` use
their existing `--config` section by default; explicit `--network-config` replaces
that section, then individual CLI flags override settings. Publish uses only its
own explicit flags/`--network-config`, never a prior operation's upstream settings.
Manual/environment modes are mutually exclusive; never silently merge contradictory
authentication sources. For password
stdin, consume one bounded UTF-8 password line before enabling optional cancel-line
processing; never echo it. The GUI library route does not need CLI secret parsing.

HTTP CONNECT proxies (`http://host:port`) carry HTTPS to the official origin;
proxy authentication is explicitly supplied Basic authentication. Basic proxy
credentials travel to an HTTP proxy without transport encryption: use a trusted
corporate network/proxy. TLS for the upstream endpoint remains mandatory. SOCKS
would enable a new dependency in ureq 2 and is deferred. IPv6-literal proxy endpoints
are refused because ureq 2's proxy parser does not support them; DNS proxy names
may resolve to IPv6. Upstream/destination origin restrictions remain unchanged.
NO_PROXY supports `*`, exact hosts, dot/domain suffixes and optional ports; match
domain boundaries, never string substrings; IP literals match exactly. CIDR and
embedded wildcard patterns are refused. Unsupported syntax is an explicit error.
No proxy or CA option can authorize a different upstream origin or a redirect.

`network::detect_environment()` returns a sanitized `EnvironmentProxy` with
`proxy_url`, parsed `no_proxy` and `credentials_present`; never include credentials
in serializable output. Selection precedence is `HTTPS_PROXY`, `https_proxy`,
`ALL_PROXY`, `all_proxy`; exclusions prefer `NO_PROXY` over `no_proxy`. Empty values
are ignored, and a malformed selected value is refused rather than silently skipped.
The API reads environment only when explicitly called. `Environment` mode is an
explicit opt-in to resolve those values; direct/manual modes never discover them.

Extra CA PEM certificates are **added to the existing Mozilla/webpki roots**,
not substituted for them. Reuse bounded PEM loading from publication, preserving
4 MiB/256-certificate limits. TLS chain/name/expiry/signature checks remain enabled.

Publishing keeps its current independent no-proxy behavior. New explicit
`publish::{preflight_with_network, publish_with_network}` APIs accept publication
options, `NetworkOptions` and `Control`; publication CLI network flags opt in to
that separate configuration. Upstream `[network]` is not silently inherited by
publishing. Explicit target-host NO_PROXY matching applies to that destination.
Existing publication `Options.ca_file`/`--ca-file` retains its private-bundle base
trust policy; network extra CAs add to that selected base. Without a private base,
Mozilla roots remain the base. CA paths are relative to the process working
directory, including paths read from TOML; GUI workers can use absolute paths.

## Diagnostics and certificate inspection

`network::inspect_ca_file(path)` returns `Vec<CertificateInfo>`: `subject`, `issuer`,
`not_after` (RFC 2822 in UTC) and `sha256` (lowercase DER fingerprint). Metadata is diagnostic,
not an instruction to trust a certificate or proof that it is a suitable CA.

`network::check_network_with_control(&NetworkOptions, &Control)` returns a
`NetworkCheckReport` with `ok` and ordered `steps`. A `CheckStep` has `name`,
`status` (`passed`, `failed`, `skipped`), nullable `error_kind`, `message` and
`certificates`. Ordered checks are `dns`, `connect`, `proxy_auth`, `tls`, `download`.
Direct connections skip proxy auth. Failure skips dependent steps and preserves
bounded, sanitized typed evidence. DNS/socket stages and timeouts must be described
accurately; socket deadlines do not imply that the platform resolver can be stopped.
The final download uses the small official stable-manifest checksum file, never a
payload archive. `check-network` exposes the same report through CLI/protocol 1.

On TLS failure, capture the chain actually presented before delegating verification
to rustls's ordinary verifier. **Never accept an invalid certificate for diagnosis.**
Return subject/issuer/expiry/fingerprint for received certificates when parseable;
roots often are not sent, so never invent an issuer's certificate or claim a full
chain. Display at most 16 parseable received certificates, 256 KiB per certificate
and 4,096 non-control characters per subject/issuer. Presenting metadata does not auto-trust or download any CA. A DNS/connect
failure can have no certificates. Malformed/oversized chains remain bounded errors.

## Download estimates

`Source::content_length` defaults to `None` for existing custom sources.
`selection::measure_sizes` takes a frozen resolved plan, source and control and
returns `SizeReport`: per-path optional `expected_bytes`, `known_bytes`,
`unknown_artifacts`, `complete`, `exceeds_budget` and `estimated: true`.
Use opt-in HEAD only; unsupported or missing lengths stay unknown, not zero.
No payload downloads or metadata re-resolution occur during measurement. A HEAD
length is an estimate, never a hash/integrity proof or permission to exceed actual
transfer budgets. Refuse malformed/contradictory lengths and checked-sum overflow.

CLI `plan --estimate-sizes` and `sync --estimate-sizes` opt in. The plan command
returns `{plan, sizes}` in events mode (equivalent TOML in human mode); estimates
are not written into the immutable frozen-plan directory. Refresh them explicitly.
`exceeds_budget: false` with unknown artifacts does not prove a budget fit.
Additive `sync::RunOptions`/`sync::run_with_options` expose that choice to library callers;
existing entry points keep their defaults. `RunOptions` contains `resume`, `force`
and `estimate_sizes`; resume/force are mutually exclusive. Sizes do not change the
frozen plan. Explicit network settings are excluded from the frozen sync recipe,
so changing proxy/CA/timeouts does not invalidate the saved selection/previous identity.
Actual downloaded/copied bytes still obey original budgets/hashes.
Sync artifact events carry known expected lengths, while aggregate progress provides
estimated totals/rate/ETA where meaningful. Unknown totals/rates/ETA are null;
reused/copied/hashed bytes must not be double-counted as new network traffic.

## Release discovery and cache

Public `releases::{load, CacheMode, ReleaseCatalog, Release}` expose stable numeric
versions/dates and beta/nightly dates. Fetch only the official root
`https://static.rust-lang.org/manifests.txt` (not `/dist/manifests.txt`). Parse only
bounded official dated manifest paths, validate dates/versions and deduplicate
records deterministically. Availability in the index is not host/component proof.
Rustup release discovery is outside this index and is not promised.

CLI `releases --cache DIRECTORY [--refresh | --offline]` supports explicit refresh
and zero-network offline reuse. Default refresh may fall back to a valid cached
index on network failure, visibly reporting the failure and cached origin/age.
`CacheMode` names are `Refresh`, `Offline`, `RefreshOrCached`. `ReleaseCatalog`
contains releases, source URL, `cached`, `fetched_at`, `age_seconds`, original-index
SHA-256 and an optional typed refresh warning. Preserve raw index bytes, bound and
validate cache contents, refuse links/unrelated state, and update atomically.
Cache hashes detect corruption, not independent upstream signature authenticity.

## Qualification and dependencies

Required evidence includes CONNECT routing and exclusions, required/denied proxy
auth, TLS inspection/private CA and retained public roots, presented-chain reporting,
CA parsing, timeouts, cancellation, redirects/foreign origins, credential redaction,
HEAD unknown/invalid lengths, estimate budgets, live index shape and offline cache
integrity. Protocol heartbeat/framing and resumable identity remain permanent gates.

The owner approved direct `webpki-roots` (already transitive in ureq's locked graph)
and `x509-parser` 0.18 with default features disabled on 2026-11-12. Parsing is
only for display; rustls retains trust, hostname, expiry and signature decisions.
See [bounded candidate evidence](evals/2026-11-12-network-030.md). New dependencies
still require owner approval. No external openssl executable is used at runtime;
tests use preinstalled OpenSSL only to generate disposable fixture certificates.

## Quick examples

```sh
rust-mirror --events jsonl check-network --proxy http://proxy.example:8080 --extra-ca-file company-ca.pem --connect-timeout 20 --read-timeout 60
rust-mirror plan --config mirror.toml --selection project --output plan-001 --estimate-sizes
rust-mirror releases --cache ./release-cache --refresh
rust-mirror releases --cache ./release-cache --offline
```

A secret-free config section:

```toml
[network]
proxy_mode = "manual"
proxy_url = "http://proxy.example:8080"
proxy_username_env = "RUST_MIRROR_PROXY_USER"
proxy_password_env = "RUST_MIRROR_PROXY_PASSWORD"
no_proxy = [".example.internal", "localhost:443"]
extra_ca_file = "company-ca.pem"
connect_timeout_seconds = 20
read_timeout_seconds = 60
```

Keep passwords in process-scoped environment variables, or supply the one password
line with `--proxy-password-stdin`; never put values into argv or saved TOML. GUI
workers construct `ProxyCredentials` directly from their private stdin channel.
Do not save it with settings or include raw authenticated URLs in diagnostics.
