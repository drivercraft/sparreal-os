#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET_JSON="./build-config/aarch64-unknown-none-softfloat-pie.json"
TARGET_STEM="$(basename "$TARGET_JSON" .json)"
ARTIFACT_DIR="./target/$TARGET_STEM/debug"
ELF_PATH="$ARTIFACT_DIR/test-suit-timer"
BIN_PATH="$ARTIFACT_DIR/test-suit-timer.bin"
QEMU_LOG="./target/qemu-aarch64-target-json.log"
QEMU_OUTPUT="./target/qemu-aarch64-target-json.out"
QEMU_TIMEOUT="${QEMU_TIMEOUT:-60s}"

cd "$PROJECT_ROOT" || exit 1
mkdir -p ./target

echo "[build] target json: $TARGET_JSON"
CARGO_ENCODED_RUSTFLAGS=$'-Clink-args=-Tlink.x' \
  cargo build \
    -p test-suit-timer \
    --target "$TARGET_JSON" \
    -Z json-target-spec \
    -Z build-std=core,alloc,compiler_builtins \
    -Z build-std-features=compiler-builtins-mem

echo "[objcopy] $BIN_PATH"
rust-objcopy -O binary "$ELF_PATH" "$BIN_PATH"

rm -f "$QEMU_LOG" "$QEMU_OUTPUT"

echo "[run] qemu-system-aarch64 (timeout: $QEMU_TIMEOUT)"
set +e
timeout --foreground "$QEMU_TIMEOUT" \
  qemu-system-aarch64 \
    -nographic \
    -cpu cortex-a53 \
    -machine virt,gic-version=3,virtualization=on \
    -d int,mmu,guest_errors \
    -smp 4 \
    -D "$QEMU_LOG" \
    -kernel "$BIN_PATH" \
  2>&1 | tee "$QEMU_OUTPUT"
qemu_status=${PIPESTATUS[0]}
set -e

if [[ "$qemu_status" -ne 0 ]]; then
  if [[ "$qemu_status" -eq 124 ]]; then
    echo "[error] qemu timed out after $QEMU_TIMEOUT" >&2
  else
    echo "[error] qemu exited with status $qemu_status" >&2
  fi
  exit "$qemu_status"
fi

if ! grep -q "All tests passed!" "$QEMU_OUTPUT"; then
  echo "[error] success marker not found in $QEMU_OUTPUT" >&2
  exit 1
fi

echo "[ok] timer test passed with target json"
