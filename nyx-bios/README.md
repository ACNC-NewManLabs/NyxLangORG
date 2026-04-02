# Nyx BIOS

Minimal x86 real-mode BIOS ROM intended for use with emulators like QEMU.

## Status

**Production-Ready for HDD Boot**

### Working Features

- ✅ Builds to exactly 128KB (131072 bytes)
- ✅ Serial output (115200 baud)
- ✅ PIC initialization (IRQs remapped 0x20-0x2F)
- ✅ A20 line enabled and verified
- ✅ IVT fully populated (all 256 vectors)
- ✅ PCI stub initialized
- ✅ ATA detection working
- ✅ Keyboard initialized (PS/2)
- ✅ SMBIOS stub initialized
- ✅ DMA 8237 initialized
- ✅ RTC initialized (CMOS)
- ✅ MBR hard disk boot working
- ✅ Boot order: HDD → CD-ROM → Network

### Known Issues

- ⚠️ ACPI initialization causes hang - currently disabled
- ⚠️ CD-ROM boot has ATAPI issue causing reset during sector read
- ⚠️ Boot menu disabled due to timeout mechanism issues

## Requirements

- `nasm`
- Optional: `qemu-system-i386` (for `make run`)

## Build

```sh
make
```

Outputs:
- `build/bios.bin` (primary artifact)
- `bios.bin` (convenience copy)

The build runs a ROM sanity check that validates:
- exact ROM size (default: 128KiB)
- reset vector location and far-jump segment (`F000`)

## Check

```sh
make check
```

## Run (QEMU)

```sh
make run
```

Serial output is routed to your terminal via `-serial stdio`.

## HDD Boot Smoke Test (QEMU)

Creates a tiny raw HDD image with a test MBR, then boots it via Nyx:

```sh
make run-hdd
```

## ISO Suite Tests (QEMU + VirtualBox)

Boot every ISO in `downloads/` using Nyx BIOS and capture serial logs:

```sh
./scripts/run_iso_suite_qemu.sh
```

VirtualBox headless suite (requires `VBoxManage`):

```sh
./scripts/run_iso_suite_vbox.sh
```

## Boot Order

The BIOS attempts to boot in this order:
1. Hard Disk (HDD) - **Working**
2. CD-ROM - (broken - causes reset)
3. Network/PXE - (stub)

## Development

The TODO.md file contains detailed development tasks and roadmap.
