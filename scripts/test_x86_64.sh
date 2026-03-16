#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT" || exit 1

SUITE="${1:-hello}"

case "$SUITE" in
  hello|timer|smp)
    ;;
  *)
    echo "Unsupported x86_64 test suite: $SUITE"
    echo "Usage: $0 [hello|timer|smp]"
    exit 1
    ;;
esac

CONFIG_FILE="./test-suit/${SUITE}/x86_64.toml"
QEMU_FILE="./test-suit/${SUITE}/qemu-x86_64.toml"

ostool run -c "$CONFIG_FILE" qemu -q "$QEMU_FILE"
