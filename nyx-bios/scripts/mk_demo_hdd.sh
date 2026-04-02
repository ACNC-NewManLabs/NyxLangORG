#!/usr/bin/env bash
set -euo pipefail

out_img="${1:-build/hdd.img}"
mbr_bin="${2:-build/mbr.bin}"

mkdir -p "$(dirname "$out_img")"

nasm -f bin -o "$mbr_bin" tools/mbr.asm

size="$(stat -c%s "$mbr_bin")"
if [[ "$size" != "512" ]]; then
  echo "error: mbr size is $size bytes, expected 512" >&2
  exit 1
fi

if command -v truncate >/dev/null 2>&1; then
  truncate -s 8M "$out_img"
else
  dd if=/dev/zero of="$out_img" bs=1M count=8 status=none
fi

dd if="$mbr_bin" of="$out_img" conv=notrunc bs=512 count=1 status=none
echo "OK: wrote demo MBR to $out_img"

