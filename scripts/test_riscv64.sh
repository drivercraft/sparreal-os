#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT" || exit 1

ostool run -c ./test-suit/timer/riscv64.toml qemu -q ./test-suit/timer/qemu-riscv64.toml
