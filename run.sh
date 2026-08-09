#!/usr/bin/env bash
cargo build --target riscv64gc-unknown-deravel.json --all --exclude deravel-kernel --exclude deravel-codegen \
  && cargo run --target riscv64gc-unknown-none-elf --bin deravel-kernel -- $@
