#!/bin/bash
set -euo pipefail
cd /src/rust-mirror
# Stable product MSRV stays 1.90; fuzz instrumentation uses this frozen nightly.
rustc --version | grep -F '1.91.0-nightly'
cargo fetch --locked --manifest-path fuzz/Cargo.toml
sha256sum fuzz/Cargo.lock > /tmp/fuzz-lock.sha256
CARGO_NET_OFFLINE=true cargo fuzz build --fuzz-dir fuzz
sha256sum --check /tmp/fuzz-lock.sha256
for target in lockfile catalog sparse_index channel_manifest events network inventory; do
    cp "fuzz/$target.options" "$OUT/$target.options"
    cp "fuzz/target/x86_64-unknown-linux-gnu/release/$target" "$OUT/$target"
    (cd "fuzz/corpus/$target" && zip -q "$OUT/${target}_seed_corpus.zip" ./*)
done
