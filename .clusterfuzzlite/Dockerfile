# Pinned builder contains Rust nightly-2025-09-05 and cargo-fuzz; no installer download.
FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:3428d0a90b1e7efbe2ba947bd1d0c9a70594986b4b6553804547f1fb24cfc92f
ENV RUSTUP_TOOLCHAIN=nightly-2025-09-05
COPY . /src/rust-mirror
WORKDIR /src/rust-mirror
