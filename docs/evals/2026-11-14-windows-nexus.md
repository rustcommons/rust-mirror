# Native Windows Nexus qualification — 2026-11-14

## Owner scope and source

The owner approved this qualification on 2026-11-13. The baseline is main
`4dba3c8d1ffbf53543142f9ace14afcac702521a` after the 0.5.0 publication record; main CI
[43513393223](https://github.com/rustcommons/rust-mirror/actions/runs/43513393223)
passed before work. Runtime source, dependencies, versions, Linux Nexus scripts and
its workflow remain unchanged. No release is implied by this test harness.

## Official native distribution and first probe

The official [win64 bundle](https://download.sonatype.com/nexus/3/nexus-3.76.1-01-win64.zip)
SHA-256 is
`8522ed370b2689667d4e6bee9414d27c10f263a7e0167af957beca1c6cdb8991`, matching
Sonatype's [published sidecar](https://download.sonatype.com/nexus/3/nexus-3.76.1-01-win64.zip.sha256).
The first native [probe 43560169060](https://github.com/rustcommons/rust-mirror/actions/runs/43560169060)
passed: Nexus/3.76.1-01 (OSS), bundled Zulu OpenJDK 17.0.10+7-LTS, temporary Raw
repository creation and normal missing-content 404 without EULA acceptance.
Owned instance/data removal and private-key destruction passed. No service or
machine-wide Java installation was made. This legacy sunset release is a protocol
compatibility fixture, not a production-version recommendation.

## Preserved initial observations

The first full [run 43560561814](https://github.com/rustcommons/rust-mirror/actions/runs/43560561814)
stopped at a test-harness assertion after a successful import. It incorrectly
expected no external-source report; the lock correctly identifies the local hexyl
root as external. All registry pins were accepted/written. The assertion now checks
that exact local-root report. This was not a runtime product defect. Cleanup passed.

The next [run 43560844191](https://github.com/rustcommons/rust-mirror/actions/runs/43560844191)
passed the complete crate path before the optional Rustup step failed with
`InvalidCertificate(UnknownIssuer)`. Windows Rustup 1.28.2's reqwest backend did not
use SSL_CERT_FILE/CURL_CA_BUNDLE for that trust decision. TLS verification stayed
on. The harness now uses the existing native Windows lifecycle proof's disposable-
runner public-CA trust route, with exact thumbprint removal and interruption cleanup.
This is a client test setup correction, not a rust-mirror runtime fix.

That run already established 79 mirrored versions / 18,108,824 archive bytes,
real partial archives after TerminateProcess, verified frozen resume, 150 payload/
metadata files plus inventory publication, no-write repeat, server overwrite refusal
and hexyl locked fetch/build plus a second fresh-target offline build. Its local
root skip, private-key/data cleanup, failed optional step and exact run identity
remain historical; they do not constitute a passing complete job.

The [d3022a5 core run 43562612307](https://github.com/rustcommons/rust-mirror/actions/runs/43562612307)
caught a Windows-only negative test in the new ZIP harness: Windows Path does not
classify a slash-rooted `/outside` as absolute. Bundle preflight now uses ZIP/POSIX
path semantics consistently on both platforms. Preflight checks original ZIP names
before Python normalizes Windows backslashes, and negative fixtures store raw names
rather than allowing the ZIP writer to rewrite them. The pinned official bundle had
already passed, and the complete native proof
[43562612316](https://github.com/rustcommons/rust-mirror/actions/runs/43562612316)
passed again, including the corrected 67-request client-only counter. This fix is
confined to the test harness; no published Rust code changes.

## Qualified native Windows run

[Run 43561690025](https://github.com/rustcommons/rust-mirror/actions/runs/43561690025)
passed at branch source `536dbd9502d260d50e90b861469801d480781306`, tested through
synthetic merge `3394952163cca56042e93461621d17ae7e026b01`. The Git trees match.
The runner image was windows-2025-vs2026, version 20261103.298.1. The
[machine-readable record](windows-nexus-20261114.json) retains the full bounded
summary, distribution identity, source and cleanup results. Final reviewed-head CI
runs/artifacts are also available on [PR #39](https://github.com/rustcommons/rust-mirror/pull/39/checks).

The dedicated job passed these gates:

- Official checksum verification and native `nexus.exe /run` using its bundled
  Zulu OpenJDK 17.0.10; no service installation or EULA acceptance/bypass.
- Random initial admin password, rotation, scoped publisher/anonymous-reader roles
  and unique new Raw repositories. Credentials never enter argv or retained logs.
- Real serde/serde_json/anyhow roots and [hexyl 0.16.0](https://github.com/sharkdp/hexyl/tree/abc20a380c8c2d9d76c1976222725d3211cef809):
  real lock digest `3343803df2472c7602d40214bfc42dff094a28ff0a25a7f02cb72ffffea18b9e`,
  dev-dependencies assert_cmd, predicates and pretty_assertions, and original project
  sources. This small CLI requires no additional SDK beyond the hosted runner's
  existing MSVC tools; no selected package was excluded or source patched.
- Archive-free saved preview/whole-plan approval: 79 versions, 18,108,824 archive
  bytes, 150 metadata/payload files plus the inventory marker in schema 5.
- A real Windows TerminateProcess during real upstream archive transfer. Nonempty
  partial files remained; resume used the same frozen ID, emitted positive-offset
  resume events and verified every completed file hash. The throttled CONNECT
  tunnel carries encrypted upstream TLS unchanged; it does not forge archives.
- Native publish preflight made no content writes. Publication used no-redeploy Raw;
  repeated publication returned typed remote_exists with no content writes. An
  intentional direct overwrite was refused and the original GET hash survived;
  anonymous writes were refused too. The final harness reads back ALLOW_ONCE/strict
  MIME policy and uses the original valid archive bytes for its direct redeploy
  control, excluding malformed-payload MIME refusal as an alternative explanation.
- Fresh isolated Windows CARGO_HOME with source replacement and test-CA-only
  CARGO_HTTP_CAINFO: cargo fetch --locked, cargo build --locked and cargo build
  --locked --offline passed with the unchanged original lock. The offline build
  used a second fresh target directory. The actual client fetched all 67 locked
  registry packages from Nexus, including dev packages. It does not compile/run
  every dev test or all three additional manual roots.
- Rustup 1.28.2 downloaded from the same native Nexus installed Rust 1.90.0 minimal
  for x86_64-pc-windows-msvc; its isolated rustc reported 1.90.0. This uses a
  separate owned Raw repository with explicit MIME relaxation for original
  toolchain signatures; the crate repository keeps strict MIME validation.
- Rustup's optional native trust setup occurred after Cargo's file-only CA proof,
  was restricted to a disposable GitHub-hosted runner, and removed its exact public
  test CA from LocalMachine ROOT with verified removal. Instance/data and private
  keys were removed; the workflow's always-run cleanup succeeded too.

The initial successful summary's archive_gets counter (147) includes publication
verification and overwrite readback. The retained request trace separates 67 actual
Cargo-client archive GETs after the anonymous-write control. The final harness
binds both its counter and dev-fetch assertion to that client-only phase; this is
a reporting/test-scope correction, not a product change.

## Reproduction, validation and limits

Follow [the Windows reproduction guide](../NEXUS-INTEGRATION.md#native-windows-qualification)
and [dedicated workflow](../../.github/workflows/nexus-windows.yml). The harness reuses
existing credentials, certificate/CRL, isolated Cargo environment, pinned-source
extraction and publisher-check logic. Linux's Docker/namespace job and scripts are
unchanged. Six focused safety tests cover ZIP paths/links, unowned paths, reused
PIDs and certificate-identity refusal. Earlier full local Python validation passed
31 tests; final CI includes the added native-trust refusal case as well.

This qualifies the exact sunset legacy server/bundled JRE and named Windows client,
not supported production deployments, Jetty-native TLS, SSO/private readers,
macOS/Linux native Nexus distributions or arbitrary crates/features/SDKs. TLS is
verified at the temporary terminator; its backend is loopback HTTP. The job has
Internet access and is not an air-gap simulation. The Rustup case is one fresh
minimal install, not a replacement for the existing four-host lifecycle matrix.
No runtime defect, release, package/dependency/source-version change or production
endpoint operation was needed. Historical 0.4.0/0.5.0 release records remain unchanged.
