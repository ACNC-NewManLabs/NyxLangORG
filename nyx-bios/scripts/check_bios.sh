#!/usr/bin/env bash
set -euo pipefail

rom_path="${1:?usage: check_bios.sh <rom.bin> <expected_size_bytes>}"
expected_size="${2:?usage: check_bios.sh <rom.bin> <expected_size_bytes>}"

if [[ ! -f "$rom_path" ]]; then
  echo "error: ROM not found: $rom_path" >&2
  exit 1
fi

actual_size="$(stat -c%s "$rom_path")"
if [[ "$actual_size" != "$expected_size" ]]; then
  echo "error: ROM size mismatch: got $actual_size bytes, expected $expected_size" >&2
  exit 1
fi

reset_off="$((expected_size - 16))"

read_hex_bytes() {
  # Prints space-separated hex bytes, lowercase, no leading/trailing spaces.
  dd if="$rom_path" bs=1 skip="$1" count="$2" status=none \
    | od -An -tx1 -v \
    | tr -s '[:space:]' ' ' \
    | sed -e 's/^ //' -e 's/ $//'
}

reset_prefix="$(read_hex_bytes "$reset_off" 6)"
# Expect: FA (CLI), EA (far JMP), XX XX (offset), 00 F0 (segment F000)
case "$reset_prefix" in
  "fa ea "*)
    ;;
  *)
    echo "error: reset vector does not start with 'FA EA' at offset $reset_off" >&2
    echo "got: $reset_prefix" >&2
    exit 1
    ;;
esac

reset_seg="$(echo "$reset_prefix" | awk '{print $(NF-1)" "$NF}')"
if [[ "$reset_seg" != "00 f0" ]]; then
  echo "error: reset far-jump segment is not F000 at offset $((reset_off + 4))" >&2
  echo "got: $reset_seg (expected: 00 f0)" >&2
  exit 1
fi

echo "OK: ROM size $actual_size bytes, reset vector present at $reset_off"

