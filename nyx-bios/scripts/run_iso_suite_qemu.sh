#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIOS_BIN="${BIOS_BIN:-$ROOT_DIR/build/bios.bin}"
ISO_DIR="${ISO_DIR:-$ROOT_DIR/downloads}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/logs/qemu}"
TIMEOUT_SECS="${TIMEOUT_SECS:-90}"

mkdir -p "$LOG_DIR"

if [[ ! -f "$BIOS_BIN" ]]; then
  echo "bios not found at $BIOS_BIN; run 'make' first" >&2
  exit 1
fi

QEMU_BIN="${QEMU_BIN:-}"
if [[ -z "$QEMU_BIN" ]]; then
  if command -v qemu-system-x86_64 >/dev/null 2>&1; then
    QEMU_BIN="qemu-system-x86_64"
  else
    QEMU_BIN="qemu-system-i386"
  fi
fi

if ! command -v "$QEMU_BIN" >/dev/null 2>&1; then
  echo "qemu not found ($QEMU_BIN). Install qemu-system-x86_64 or qemu-system-i386." >&2
  exit 1
fi

run_with_timeout() {
  local cmd=("$@")
  if command -v timeout >/dev/null 2>&1; then
    timeout --preserve-status "${TIMEOUT_SECS}s" "${cmd[@]}"
  else
    "${cmd[@]}" &
    local pid=$!
    sleep "$TIMEOUT_SECS"
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      sleep 1
      kill -9 "$pid" 2>/dev/null || true
    fi
  fi
}

shopt -s nullglob
isos=("$ISO_DIR"/*.iso)
if [[ ${#isos[@]} -eq 0 ]]; then
  echo "no ISO files found in $ISO_DIR" >&2
  exit 1
fi

for iso in "${isos[@]}"; do
  base="$(basename "$iso" .iso)"
  log="$LOG_DIR/${base}.serial.log"
  echo "[QEMU] Booting $iso"
  run_with_timeout "$QEMU_BIN" \
    -bios "$BIOS_BIN" \
    -m 1024 \
    -drive file="$iso",format=raw,media=cdrom,if=ide,index=2 \
    -display none \
    -monitor none \
    -serial "file:$log" \
    -no-reboot -no-shutdown || true
  echo "[QEMU] Serial log: $log"
  echo
  sleep 1
done
