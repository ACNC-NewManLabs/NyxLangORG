# nyx-bios/Makefile

NASM ?= nasm
NASMFLAGS ?= -f bin -w+all

BUILD_DIR ?= build
DOWNLOADS_DIR ?= downloads
# Must match the ROM layout constants in src/main.asm.
ROM_SIZE := 131072

# Prefer the distro QEMU binary (avoids Snap runtime issues on some systems).
QEMU ?= qemu-system-i386
QEMU_BIN := $(if $(wildcard /usr/bin/qemu-system-i386),/usr/bin/qemu-system-i386,$(QEMU))

OUT := $(BUILD_DIR)/bios.bin
ISO ?=

ASM_SRCS := $(wildcard src/*.asm)

all: bios.bin

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(OUT): $(ASM_SRCS) | $(BUILD_DIR)
	$(NASM) $(NASMFLAGS) -o $(OUT) src/main.asm
	@./scripts/check_bios.sh $(OUT) $(ROM_SIZE)

bios.bin: $(OUT)
	cp -f $(OUT) bios.bin
	@echo "BUILD OK - BIOS ready ($$(stat -c%s bios.bin) bytes)"

check: $(OUT)

clean:
	rm -rf $(BUILD_DIR)
	rm -f bios.bin

run: $(OUT)
	$(QEMU_BIN) -bios $(OUT) -display none -monitor none -serial stdio -no-reboot -no-shutdown

hexdump-reset: $(OUT)
	@./scripts/hexdump_reset.sh $(OUT) $(ROM_SIZE)

demo-hdd: $(OUT)
	@./scripts/mk_demo_hdd.sh $(BUILD_DIR)/hdd.img $(BUILD_DIR)/mbr.bin

run-hdd: demo-hdd
	$(QEMU_BIN) -bios $(OUT) -drive file=$(BUILD_DIR)/hdd.img,format=raw,if=ide,index=0 -display none -monitor none -serial stdio -no-reboot -no-shutdown

fetch-alpine:
	@./scripts/fetch_alpine_iso.sh $(DOWNLOADS_DIR)/alpine.iso

run-iso: $(OUT)
	@test -n "$(ISO)" || (echo "error: set ISO=path/to/alpine.iso" >&2; exit 2)
	@test -f "$(ISO)" || (echo "error: ISO not found: $(ISO)" >&2; exit 2)
	$(QEMU_BIN) -bios $(OUT) -drive file=$(ISO),format=raw,media=cdrom,if=ide,index=2 -display none -monitor none -serial stdio -no-reboot -no-shutdown

.PHONY: all check clean run hexdump-reset demo-hdd run-hdd fetch-alpine run-iso
