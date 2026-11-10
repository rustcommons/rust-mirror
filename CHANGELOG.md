# Changelog

## Unreleased

- Add named version/channel, host, profile, component and cross-target selections.
- Add release-specific discovery and explicit-host project toolchain-file import.
- Split metadata-only planning from verified frozen-plan downloads; mirror all
  advertised archive formats for selected components.
- Support schema 2 selected snapshots in the publisher and isolated Linux proof.

- Add an experimental CLI/library for pinned, minimal-profile Linux toolchain snapshots.
- Preserve upstream manifests and signature files; check downloaded SHA-256 values.
- Add static-host/Nexus Raw publication helper and isolated offline-install proof.
- No version has been released. Cargo crate mirroring and the GUI remain planned.

- Add publisher `--ca-file` for verified private-CA HTTPS connections.
- Add a disposable actual-Nexus OSS compatibility proof with scoped access, MIME-policy controls and isolated offline installation; keep the Community Edition EULA gate explicit.
