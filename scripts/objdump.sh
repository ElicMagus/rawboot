#!/usr/bin/env bash
# SPDX-License-Identifier: Unlicense
#
# Run llvm-objdump from the llvm-tools component. There is no `cargo objdump`;
# the binary lives under the toolchain's lib/rustlib, not on PATH.
set -euo pipefail

sysroot=$(rustc --print sysroot)
objdump=$(find "$sysroot" -type f -name llvm-objdump -print -quit)
if [ -z "$objdump" ]; then
    echo "error: llvm-objdump not found under $sysroot; add the llvm-tools component" >&2
    exit 2
fi

exec "$objdump" "$@"
