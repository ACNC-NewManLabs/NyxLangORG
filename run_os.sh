#!/bin/bash
set -e

echo "[ Nyx OS Packager ]"

# Construct the GRUB filesystem structure
mkdir -p build/isodir/boot/grub

# Copy our compiled bare-metal kernel
cp build/kernel build/isodir/boot/nyx_kernel.bin

# Create the grub.cfg
cat > build/isodir/boot/grub/grub.cfg << EOF
menuentry "Nyx Operating System" {
	multiboot2 /boot/nyx_kernel.bin
	boot
}
EOF

# Build the ISO using grub-mkrescue
grub-mkrescue -o build/nyx_os.iso build/isodir

echo "ISO successfully built at build/nyx_os.iso"

# Emulate!
qemu-system-x86_64 -cdrom build/nyx_os.iso -monitor stdio -display none -serial file:qemu.log -d int,cpu_reset -no-reboot & 
QEMU_PID=$!
sleep 3
kill $QEMU_PID
echo "(QEMU emulator completed)"
