#!/usr/bin/env bash

(mkdir -p disk && cd disk && tar cf ../disk.tar --format=ustar *.txt)

#timeout -f 5 \
qemu-system-riscv64 \
    -machine virt \
    -bios default \
    -serial mon:stdio \
    -device pci-serial,chardev=pciuart \
    -chardev file,id=pciuart,path=uart.txt \
    -drive id=drive0,file=disk.tar,format=raw,if=none \
    -device virtio-blk-pci,drive=drive0,disable-legacy=on \
    -netdev user,id=net0,net=192.168.100.0/24,host=192.168.100.1 \
    -device virtio-net-pci,netdev=net0,disable-legacy=on \
    -object filter-dump,id=f1,netdev=net0,file=dump.dat \
    -device virtio-gpu \
    -display gtk,full-screen=off \
    -device virtio-keyboard \
    -device virtio-mouse \
    --no-reboot \
    -kernel \
    $@
