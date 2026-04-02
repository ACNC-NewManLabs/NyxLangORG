#!/usr/bin/env bash
set -euo pipefail

rom_path="${1:?usage: hexdump_reset.sh <rom.bin> <rom_size_bytes>}"
rom_size="${2:?usage: hexdump_reset.sh <rom.bin> <rom_size_bytes>}"

if [[ ! -f "$rom_path" ]]; then
  echo "error: ROM not found: $rom_path" >&2
  exit 1
fi

actual_size="$(stat -c%s "$rom_path")"
if [[ "$actual_size" -lt 16 ]]; then
  echo "error: ROM too small ($actual_size bytes)" >&2
  exit 1
fi

reset_off="$((rom_size - 16))"
if [[ "$actual_size" != "$rom_size" ]]; then
  echo "warning: ROM size is $actual_size bytes, expected $rom_size; using actual end-of-file offset" >&2
  reset_off="$((actual_size - 16))"
fi

echo "Reset vector @ file offset $reset_off (16 bytes):"
dd if="$rom_path" bs=1 skip="$reset_off" count=16 status=none | od -An -tx1 -v

