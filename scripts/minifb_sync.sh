#!/usr/bin/env bash
set -euo pipefail

UPSTREAM_VERSION="0.28.0"
REGISTRY_PATH="$HOME/.cargo/registry/src"

if [ ! -d "$REGISTRY_PATH" ]; then
  echo "Cargo registry not found at $REGISTRY_PATH" >&2
  exit 1
fi

SRC=$(find "$REGISTRY_PATH" -maxdepth 2 -type d -name "minifb-$UPSTREAM_VERSION" | head -n 1)
if [ -z "$SRC" ]; then
  echo "minifb $UPSTREAM_VERSION not found in registry; run 'cargo fetch' first" >&2
  exit 1
fi

rm -rf third_party/minifb
cp -a "$SRC" third_party/minifb

# Re-apply local patch: replace instant with web-time for wasm32.
perl -0pi -e 's/\[target.\x27cfg\(target_arch = "wasm32"\)\x27\]\.dependencies\.instant\nversion = "0\.1\.12"\nfeatures = \[\n\s*"wasm-bindgen",\n\s*"inaccurate",\n\]\n/\[target.\x27cfg\(target_arch = "wasm32"\)\x27\]\.dependencies\.web-time\nversion = "1\.1\.0"\n/sg' third_party/minifb/Cargo.toml
perl -0pi -e 's/extern crate instant;\n\n#\[cfg\(target_arch = "wasm32"\)\]\nuse instant::\{Duration, Instant\};/use web_time::\{Duration, Instant\};/sg' third_party/minifb/src/key_handler.rs
perl -0pi -e 's/extern crate instant;\n#\[cfg\(target_arch = "wasm32"\)\]\nuse instant::\{Duration, Instant\};/use web_time::\{Duration, Instant\};/sg' third_party/minifb/src/rate.rs

# Silence upstream lint noise introduced by recent toolchains.
perl -0pi -e 's/#!\[deny\(missing_debug_implementations\)\]/#!\[deny\(missing_debug_implementations\)\]\n#!\[allow\(mismatched_lifetime_syntaxes\)\]/' third_party/minifb/src/lib.rs

echo "minifb synced and patched." 
