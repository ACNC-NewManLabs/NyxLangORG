#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIOS_BIN="${BIOS_BIN:-$ROOT_DIR/build/bios.bin}"
ISO_DIR="${ISO_DIR:-$ROOT_DIR/downloads}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/logs/vbox}"
TIMEOUT_SECS="${TIMEOUT_SECS:-90}"
VM_NAME="${VM_NAME:-NyxBIOS-ISO-Test}"

mkdir -p "$LOG_DIR"

if [[ ! -f "$BIOS_BIN" ]]; then
  echo "bios not found at $BIOS_BIN; run 'make' first" >&2
  exit 1
fi

if ! command -v VBoxManage >/dev/null 2>&1; then
  echo "VBoxManage not found. Install VirtualBox." >&2
  exit 1
fi

if ! VBoxManage list vms | grep -q "\"$VM_NAME\""; then
  VBoxManage createvm --name "$VM_NAME" --register
  VBoxManage modifyvm "$VM_NAME" --memory 2048 --cpus 2 --firmware bios --boot1 dvd
  VBoxManage modifyvm "$VM_NAME" --uart1 0x3F8 4
  VBoxManage storagectl "$VM_NAME" --name "IDE" --add ide --controller PIIX4
fi

# Set custom BIOS image
VBoxManage setextradata "$VM_NAME" "VBoxInternal/Devices/pcbios/0/Config/CustomBIOS" "$BIOS_BIN"

shopt -s nullglob
isos=("$ISO_DIR"/*.iso)
if [[ ${#isos[@]} -eq 0 ]]; then
  echo "no ISO files found in $ISO_DIR" >&2
  exit 1
fi

for iso in "${isos[@]}"; do
  base="$(basename "$iso" .iso)"
  log="$LOG_DIR/${base}.serial.log"

  echo "[VBox] Booting $iso"

  # Ensure VM is powered off
  VBoxManage controlvm "$VM_NAME" poweroff >/dev/null 2>&1 || true

  # Attach ISO
  VBoxManage storageattach "$VM_NAME" --storagectl "IDE" --port 0 --device 0 --type dvddrive --medium "$iso"

  # Route serial to file
  VBoxManage modifyvm "$VM_NAME" --uartmode1 file "$log"

  # Start headless
  VBoxManage startvm "$VM_NAME" --type headless

  # Let it boot for a while, then power off
  sleep "$TIMEOUT_SECS"
  VBoxManage controlvm "$VM_NAME" poweroff >/dev/null 2>&1 || true

  echo "[VBox] Serial log: $log"
  echo
  sleep 1

done

