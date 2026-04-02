# Nyx BIOS Production Readiness TODO
Generated from approved plan to make BIOS fully production-ready (Tier 1+ fixes).

## Priority 1: Critical Fixes (Boot All Linux ISOs)
- [x] Diagnosed: PVD read hang LBA=16. Prep fixes: drain+retry+log. In-progress.
- [ ] Enable boot menu basic (boot_menu.asm): Timeout → HDD fallback.

## Priority 2: Tier 1 Linux Distros
- [ ] Polish E820/INT15 (memory.asm/main.asm): Full map, E801/88/E820/ACPI reserve.
- [ ] INT1A RTC time (rtc.asm/main.asm): Get BCD time/date.
- [ ] INT13 extras (disk.asm): Geometry 1023/255/63, write-protect, NMI.

## Priority 3: Tier 2 Windows Stubs
- [ ] ACPI DSDT/SSDT (acpi.asm): Expand \_S5, CPU C-states.
- [ ] SMBIOS types 3/4/7/9 (smbios.asm): Chassis/CPU/cache/slots.

## Priority 4: Polish & Hardening
- [ ] PCI INT1A B1xx (pci.asm).
- [ ] BIOS checksum, stack canary, PIT watchdog (main.asm).
- [ ] Zero raw hex audit.

## Priority 5: Verification
- [ ] Run scripts/run_iso_suite_qemu.sh → All Linux PASS.
- [ ] 100 QEMU boots no-crash.
- [ ] Update README/TODO ✅.

**Progress: 0/X complete.** Edit this file as steps finish.
