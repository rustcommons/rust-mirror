# 0.3.0 network planning — 2026-11-12

This is source/planning evidence, not implementation qualification.

- Installed ureq 2.12.1 source: `rtls.rs` builds private webpki default roots; the
  public custom-TLS route requires supplying a config. `proxy.rs` supports HTTP
  CONNECT/Basic auth. SOCKS enables optional `socks`, so it is deferred.
- [Rust Forge channel layout](https://forge.rust-lang.org/infra/channel-layout.html)
  identifies the official manifest index and dated channel paths. Live root index
  returned HTTP 200, 333292 bytes, SHA-256
  `3ed90e48ea4437ec5130eb9aace3dc76a5788b3a462bef4eb143a5f9635e4d88`; `/dist/manifests.txt` returned 404.
  Parsed 139 dated full-numeric stable records, 638 dated beta aliases and
  3669 dated nightly aliases. These are index-shape observations, not
  release/client/component verification. Numeric beta aliases also exist; they
  must not be misclassified as stable numeric versions.
- A small official `dist/channel-rust-stable.toml.sha256` request returned 200
  with 91 bytes, suitable for a bounded connection check.
- [x509-parser 0.18 documentation](https://docs.rs/x509-parser/0.18.0/x509_parser/)
  exposes subject/issuer/validity parsing separately from optional signature
  verification; runtime trust decisions will remain with rustls.

Dependency approval requested: direct access to existing locked `webpki-roots`
plus `x509-parser` with default features disabled. Neither was added. TLS
inspection/proxy fixtures, default-root retention and credential redaction remain
unverified until implementation. Publication is not authorized in this task.

## Subsequent authorization and implementation

On 2026-11-12 the owner approved both dependencies. The implementation and bounded
fixture/live observations are tracked in the [0.3.0 candidate record](2026-11-12-network-030.md).
The planning observations above remain historical; they do not certify the candidate.
