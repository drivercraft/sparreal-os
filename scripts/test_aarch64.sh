#!/bin/bash
# 获取脚本所在目录的父目录（项目根目录）
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 切换到项目根目录
cd "$PROJECT_ROOT" || exit 1

# 执行测试脚本
"$SCRIPT_DIR/test_aarch64_el1.sh"
"$SCRIPT_DIR/test_aarch64_el2.sh"
"$SCRIPT_DIR/test_aarch64_smp.sh"
