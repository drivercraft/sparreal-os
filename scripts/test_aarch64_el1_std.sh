#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET_JSON="./build-config/aarch64-unknown-linux-musl-softfloat-pie.json"
TARGET_STEM="$(basename "$TARGET_JSON" .json)"
PROFILE="${PROFILE:-release}"
ARTIFACT_DIR="$PROJECT_ROOT/target/$TARGET_STEM/$PROFILE"
ELF_PATH="$ARTIFACT_DIR/test-suit-std-smoke"
BIN_PATH="$ARTIFACT_DIR/test-suit-std-smoke.bin"
SHIM_LIB_DIR="$PROJECT_ROOT/target/$TARGET_STEM/shim-libs"
QEMU_LOG="$PROJECT_ROOT/target/qemu-aarch64-std.log"
QEMU_OUTPUT="$PROJECT_ROOT/target/qemu-aarch64-std.out"
QEMU_TIMEOUT="${QEMU_TIMEOUT:-60s}"

cd "$PROJECT_ROOT" || exit 1
mkdir -p "$SHIM_LIB_DIR"

CARGO_PROFILE_ARG=()
if [[ "$PROFILE" == "release" ]]; then
  CARGO_PROFILE_ARG+=(--release)
elif [[ "$PROFILE" != "debug" ]]; then
  echo "[error] unsupported PROFILE: $PROFILE" >&2
  exit 1
fi

DUMMY_C="$SHIM_LIB_DIR/dummy.c"
DUMMY_O="$SHIM_LIB_DIR/dummy.o"
cat > "$DUMMY_C" <<'EOF'
void __sparreal_std_shim_dummy(void) {}
EOF

clang --target=aarch64-unknown-linux-musl -c "$DUMMY_C" -o "$DUMMY_O"
for lib in c gcc_s; do
  ar rcs "$SHIM_LIB_DIR/lib${lib}.a" "$DUMMY_O"
done

echo "[build] target json: $TARGET_JSON"
CARGO_ENCODED_RUSTFLAGS=$'-Clink-args=-Tlink.x\x1f-Lnative='"$SHIM_LIB_DIR" \
  cargo +nightly build \
    "${CARGO_PROFILE_ARG[@]}" \
    -p test-suit-std-smoke \
    --target "$TARGET_JSON" \
    -Z json-target-spec \
    -Z build-std=std,panic_abort \
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

if ! grep -q "All std smoke tests passed!" "$QEMU_OUTPUT"; then
  echo "[error] success marker not found in $QEMU_OUTPUT" >&2
  exit 1
fi

echo "[ok] std smoke test passed"
