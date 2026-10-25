#!/usr/bin/env bash
# SPDX-License-Identifier: Unlicense
#
# The repo's single check entry point, run locally and by CI.
set -euo pipefail

cd "$(dirname "$0")/.."

host_target=x86_64-unknown-linux-gnu

echo "==> cargo fmt --check"
cargo fmt --all -- --check

echo "==> SPDX headers"
missing=$(
  git ls-files --cached --others --exclude-standard '*.rs' |
    while read -r file; do
      grep -q 'SPDX-License-Identifier' "$file" || echo "$file"
    done
)
if [ -n "$missing" ]; then
  echo "missing SPDX-License-Identifier in:" >&2
  echo "$missing" >&2
  exit 1
fi

echo "==> branch docs rule"
bash scripts/check-branch-docs.sh

echo "==> no crate-level allow(warnings)"
blanket=$(
  git ls-files --cached --others --exclude-standard '*.rs' |
    while read -r file; do
      if grep -Eq '^[[:space:]]*#!\[allow\((warnings|unused)\)\]' "$file"; then
        echo "$file"
      fi
    done
)
if [ -n "$blanket" ]; then
  echo "crate-level allow(warnings/unused) is forbidden (use targeted allows):" >&2
  echo "$blanket" >&2
  exit 1
fi

echo "==> shellcheck (scripts)"
if command -v shellcheck >/dev/null 2>&1; then
  shellcheck scripts/*.sh
else
  echo "skip: shellcheck not installed (CI installs it)"
fi

echo "==> python tests (scripts)"
python3 -m unittest discover -s scripts -p 'test_*.py'

echo "==> cargo build --release"
cargo build --release

echo "==> cargo clippy (bare-metal)"
cargo clippy -- -D warnings

echo "==> cargo clippy (sunxi-drivers, write)"
cargo clippy -p sunxi-drivers -- -D warnings

echo "==> cargo clippy (sunxi-drivers, read-only)"
cargo clippy -p sunxi-drivers --no-default-features -- -D warnings

echo "==> cargo clippy (core_io shim)"
cargo clippy --manifest-path vendor/core_io/Cargo.toml --target-dir target/core_io-shim -- -D warnings

echo "==> cargo clippy (host tooling)"
cargo clippy -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers --target "$host_target" --all-targets -- -D warnings

echo "==> cargo test (host)"
cargo test -p rawboot-image -p rawboot-fel -p xtask -p sunxi-drivers --target "$host_target"

echo "==> cargo xtask image"
cargo xtask image

echo "==> loader carries no write feature or write symbols"
if cargo tree -p rawboot -e features | grep 'sunxi-drivers feature "write"' >/dev/null; then
    echo "error: rawboot resolved sunxi-drivers/write; the loader must build without it" >&2
    exit 1
fi
if scripts/objdump.sh -t target/armv7a-none-eabi/release/rawboot | grep 'sdmmc_blk_write' >/dev/null; then
    echo "error: rawboot ELF contains sdmmc_blk_write; the write path leaked into the loader" >&2
    exit 1
fi

echo "==> cargo xtask payload"
cargo xtask payload

echo "==> cargo xtask flasher"
cargo xtask flasher

echo "==> cargo xtask sd-image"
cargo xtask sd-image --partition-sectors 1843200

echo "all checks passed"
