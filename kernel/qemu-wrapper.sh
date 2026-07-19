#!/usr/bin/env bash

(mkdir -p disk && cd disk && tar cf ../disk.tar --format=ustar *.txt)

if [[ "${DERAVEL_QEMU}" == *"-S"* ]] ; then
    DERAVEL_FULL_SCREEN=off
else
    DERAVEL_FULL_SCREEN=on
fi

qemu-system-riscv64 \
    ${DERAVEL_QEMU} \
    -machine virt \
    -bios default \
    -serial mon:stdio \
    -drive id=drive0,file=disk.tar,format=raw,if=none \
    -device virtio-blk-pci,drive=drive0,disable-legacy=on \
    -netdev user,id=net0,net=192.168.100.0/24,host=192.168.100.1 \
    -device virtio-net-pci,netdev=net0,disable-legacy=on \
    -device virtio-gpu \
    -display gtk,full-screen=${DERAVEL_FULL_SCREEN},show-cursor=on,gl=on \
    -device virtio-keyboard \
    -device virtio-tablet \
    --no-reboot \
    -kernel \
    $@
