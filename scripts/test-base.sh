#!/bin/bash

# 获取脚本所在目录的父目录（项目根目录）
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 切换到项目根目录
cd "$PROJECT_ROOT" || exit 1

# 运行测试
ostool run -c ./test-suit/hello/loongarch64.toml qemu -q ./test-suit/hello/qemu-la64.toml
ostool run -c ./test-suit/hello/aarch64.toml qemu -q ./test-suit/hello/qemu-aarch64.toml