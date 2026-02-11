#!/usr/bin/env bash

(cd disk && tar cf ../disk.tar --format=ustar *.txt)

qemu-system-riscv64 \
    -machine virt \
    -bios default \
    -nographic \
    -serial mon:stdio \
    -drive id=drive0,file=disk.tar,format=raw,if=none \
    -device virtio-blk-device,drive=drive0,bus=virtio-mmio-bus.0 \
    --no-reboot \
    -kernel \
    $@

# -netdev tap,id=net0,ifname=tap0,script=no,downscript=no \
# -device virtio-net-device,netdev=net0,bus=virtio-mmio-bus.1 \
# -object filter-dump,id=f1,netdev=net0,file=dump.dat \
