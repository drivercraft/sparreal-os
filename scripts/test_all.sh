#!/bin/bash

# 获取脚本所在目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 执行测试脚本
"$SCRIPT_DIR/test_aarch64.sh"
"$SCRIPT_DIR/test_loongarch64.sh"
"$SCRIPT_DIR/test_aarch64_el2.sh"