#!/usr/bin/env bash
set -euo pipefail

export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-1700000000}

cargo check --workspace --all-targets
cargo test --workspace

cargo build --release
sha256sum target/release/nyx > /tmp/nyx_release_sha1.txt

cargo clean
cargo build --release
sha256sum target/release/nyx > /tmp/nyx_release_sha2.txt

if ! diff -u /tmp/nyx_release_sha1.txt /tmp/nyx_release_sha2.txt; then
  echo "Release build is not reproducible across two local builds" >&2
  exit 1
fi
