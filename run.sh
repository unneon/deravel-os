#!/usr/bin/env bash

(mkdir -p disk && cd disk && tar cf ../disk.tar --format=ustar *.txt)

if [[ -n "${DERAVEL_GDB+x}" ]] ; then
    export DERAVEL_GDB_FLAGS="-S -gdb tcp::${DERAVEL_GDB}"
else
    export DERAVEL_GDB_FLAGS=""
fi

if [[ -n "${DERAVEL_TIMEOUT+x}" ]] ; then
    export DERAVEL_TIMEOUT_CMD="timeout -f ${DERAVEL_TIMEOUT}"
else
    export DERAVEL_TIMEOUT_CMD=""
fi

${DERAVEL_TIMEOUT_CMD} qemu-system-riscv64 \
    ${DERAVEL_GDB_FLAGS} \
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
    -display gtk,full-screen=on,show-cursor=on \
    -device virtio-keyboard \
    -device virtio-mouse \
    --no-reboot \
    -kernel \
    $@
