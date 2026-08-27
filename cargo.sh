#!/usr/bin/env bash
set -e
case $1 in
  build)
    shift 1
    cargo build --target riscv64gc-unknown-deravel.json --all --exclude deravel-kernel --exclude deravel-codegen $@
    cargo build --target riscv64gc-unknown-none-elf --package deravel-kernel $@
    ;;
  run)
    shift 1
    cargo build --target riscv64gc-unknown-deravel.json --all --exclude deravel-kernel --exclude deravel-codegen $(printf "%s\n" "$@" | sed '/^--$/,$d')
    cargo run --target riscv64gc-unknown-none-elf --package deravel-kernel $@
    ;;
  clippy)
    shift 1
    cargo clippy --target riscv64gc-unknown-deravel.json --all --exclude deravel-kernel --exclude deravel-codegen $@
    cargo clippy --target riscv64gc-unknown-none-elf --package deravel-kernel $@
    ;;
  *)
    cargo $@
    ;;
esac
