#!/bin/bash
# $env:KERNEL_BUILTIN_CMDLINE = "earlycon=pl011,mmio32,0x9000000"

# 获取脚本所在目录的父目录（项目根目录）
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 切换到项目根目录
cd "$PROJECT_ROOT" || exit 1

# 运行测试
ostool run -c ./test-suit/smp/aarch64.toml qemu -q ./test-suit/smp/qemu-aarch64.toml