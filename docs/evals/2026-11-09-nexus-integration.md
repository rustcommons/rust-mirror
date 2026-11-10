# Actual Nexus Raw compatibility proof - 2026-11-09

## Disposition

**Legacy OSS profile: passed, with explicit repository MIME policy.** The pinned
Nexus Repository OSS 3.76.1-01 container accepted an authenticated snapshot upload,
served byte-identical artifacts over a verified HTTPS proxy, and supplied a fresh
Rustup 1.28.2 / Rust 1.90.0 minimal x86_64 Linux installation. The resulting Cargo
built and ran a dependency-free application offline. All 18 recorded cases passed;
all seven client commands exited zero. There were 48 recorded TLS requests, including
intentional 400/401/403/404 controls; not every request was expected to succeed.

**Community Edition 3.90.6-01: held.** Its content endpoints returned HTTP 403 with
an explicit EULA-onboarding message. No agreement was accepted, no acceptance API
was called and no content validation/installation pass is claimed for this edition.
See [the observed gate](2026-11-09-nexus-ce-gate.json) and Sonatype's
[onboarding policy](https://help.sonatype.com/en/ce-onboarding.html).

OSS 3.76.1 is a [sunset version](https://help.sonatype.com/en/sonatype-nexus-repository-3-versions-status.html).
It is a backwards-compatibility fixture, not a recommended production server version
or an alternative deployment proposed to avoid newer license terms. No production
instance was contacted or configured. Your real instance still needs its own version,
certificate/access/deployment-policy acceptance.

## Source and runtime identity

- Prior milestone PR #6 was **squash merged** as
  [`d66858c9dbaae7e72227b8b3ec8572a2ff1e24f0`](https://github.com/rustcommons/rust-mirror/commit/d66858c9dbaae7e72227b8b3ec8572a2ff1e24f0).
  Its tree equals qualified `f73976227e999cc36214d6410e8b70a4fc46e9fa`.
- Rust core, CLI, Cargo manifest and dependency lock are unchanged by this iteration.
  The snapshot/inventory is the same verified dataset from the prior milestone.
- OSS source image: `docker.io/sonatype/nexus3:3.76.1`;
  registry digest `sha256:390b8ec7e712c680ed8b449b766695a7c5ebbf41e489317618e63ca199f2cc09`;
  image ID `3a93fe55b65da099fcd1cb4a685ad882efe45d521affc342c700f395d9d27645`.
- Observed server header: `Nexus/3.76.1-01 (OSS)`.
- Runtime: rootful Podman in the existing Ubuntu WSL userland, CPU limit 2, container
  memory limit 2 GB, dedicated ephemeral data volume, **network mode none**.
- Client/proxy joined only that container's network namespace. Only `lo` existed,
  no routes were present, and a numeric external connection was refused.
- No image port was published to the host. The temporary TLS proxy forwarded HTTPS
  to the actual Nexus HTTP listener on isolated loopback; this is not Nexus-native TLS.

[Final summary](2026-11-09-nexus-summary.json) and
[runner identity/cleanup](2026-11-09-nexus-runner.json) retain the exact cases and
resource identities. The repository retains no private keys, test passwords,
installed toolchains or container data. Passwords were generated solely for the
owned test instance; initial-password files and private TLS key directories were
removed. The runner verified resource labels before removing its container/volume.

## Cases

| Case | Observed result |
| --- | --- |
| Actual Nexus REST readiness | OSS 3.76.1-01, successful status endpoint. |
| Raw hosted repository | Configured `ALLOW_ONCE`, confirmed by API. |
| Publisher/reader roles | Distinct local identities; scoped permissions for one repository. |
| Anonymous upload | HTTP 401, refused. |
| Wrong publishing password | HTTP 401, refused. |
| Publisher into another repository | HTTP 403, refused. |
| Untrusted test CA | Certificate verification failed. |
| Wrong hostname with trusted CA | Hostname verification failed. |
| Original upstream signature with strict MIME | HTTP 400, rejected. |
| Explicit test-only arbitrary-MIME opt-in | Dedicated repository changed to non-strict MIME; `ALLOW_ONCE` retained. |
| Same signature bytes after policy selection | HTTP 201 and exact GET identity; no rewriting/conversion. |
| Authenticated snapshot publication | Nine files PUT and GET-verified, including completion inventory. |
| Repeating publisher | Refused at existing-destination preflight; no new PUT. |
| Direct server overwrite attempt | HTTP 400; original bytes remained unchanged. |
| Missing artifact | HTTP 404. |
| Anonymous read from another repository | HTTP 401, refused. |
| Fresh anonymous Rustup bootstrap/install | Trusted HTTPS, empty dedicated homes, correct pinned compiler. |
| Dependency-free offline build/run | Cargo exit 0; program printed `nexus offline proof ok`. |

The production publisher does not configure roles, accept an EULA or change MIME
policy. The new `--ca-file` selects a trusted PEM bundle while keeping chain and
hostname verification. The destructive server setup belongs only to the explicitly
named disposable proof scripts, whose target is fixed to namespace-local loopback.

## Findings preserved from earlier attempts

The successful OSS run is a **fresh complete run**, not a resumed combination of
failed attempts. The earlier results remain retained separately:

1. CE setup initially attempted to edit its built-in anonymous identity; the API
   returned 400. The fixture now creates a separate read-only identity and selects
   that for anonymous access. No production account was edited.
2. CE attempts first looked like a permission failure because content HEAD returned
   403 for administrator, publisher and anonymous alike. Direct GET exposed the
   EULA requirement. Authentication/TLS controls passed in bounded scope, but these
   were not an upload/install pass. The harness now recognizes the gate and stops;
   it contains no EULA-acceptance endpoint.
3. The first OSS upload reached the unchanged `.asc` and failed with 400. Nexus's
   retained server log reports: detected `text/plain`, expected
   `application/pgp-signature`. The header value alone does not control this content
   versus filename comparison, as the [Nexus validator source](https://github.com/sonatype/nexus-public/blob/0a8a425daa4b37e924ca11e4637a41afce7b115c/public/common/components/nexus-repository-services/src/main/java/org/sonatype/nexus/repository/mime/DefaultContentValidator.java)
   also explains. Signatures are retained unchanged; OpenPGP is not independently
   verified by this tool.
4. An initial MIME probe expected the detailed rejection in the HTTP response body;
   that body did not include it. The final probe records HTTP 400, explicitly opts
   into the dedicated repository's MIME setting, repeats the same request and
   verifies the accepted bytes. Exact mismatch detail is retained from server logs.

Sonatype [documents the Raw MIME option](https://help.sonatype.com/en/raw-repositories.html).
Disabling it removes that server-side content/extension check; it does not disable
TLS, scoped permissions, no-redeploy policy or the publisher's mandatory hash checks.
The test changes only its own new repository and requires `--allow-raw-mime-types`.
Operators must make their own production policy decision rather than the publisher
silently weakening a server on HTTP 400.

## Validation and reproduction

- Windows Python 3.12: eight preflight tests pass, one symlink test is explicitly
  skipped because this account cannot create symlinks.
- Linux Python 3.12: all nine preflight tests pass, including symlink refusal.
- Rust source/lock unchanged; the prior 15-test Linux/Windows qualification and
  source CI carry forward. The new PR's CI independently runs the standard checks.
- The real Nexus proof checks custom CA trust, hostname rejection and authenticated
  streaming against the actual product, rather than only mocked HTTP responses.
- The older HTTP-fixture proof remains separately identified as historical evidence.

[Reproduction instructions and parameter meanings](../NEXUS-INTEGRATION.md) define
image selection, root/namespace prerequisites, generated credentials, test-only MIME
policy, isolated evidence, TLS termination and cleanup. The manual legacy-compatibility
workflow is provided for future repetition; it was not dispatched for this local run.

## Remaining boundaries

Only this legacy server/image, Raw configuration, trusted test CA, Basic publishing,
anonymous read-only Rustup access and pinned Linux toolchain/client are demonstrated.
Community Edition upload/install remains held for the owner's onboarding decision.
Modern supported server versions, your deployed reverse proxy/CA/auth setup, password-
protected Rustup downloads, token/SSO variants, signed transfer trust, alternate
platforms/components, crate/index mirroring, GUI and production deployment remain
outside this result. No software release or registry publication was performed.

## Retained proof packet

The sanitized proof packet contains 20 members, 5549 bytes;
SHA-256 `fd0edafeff683446078bd3c348ade826d141376abc95bb084d8eca53effcf63b`. It contains whitelisted summaries/logs and the public test CA, excluding credentials, private keys, server data and installed executables. It is retained in coordinator thread storage.
