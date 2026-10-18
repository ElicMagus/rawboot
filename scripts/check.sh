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

echo "==> cargo build --release"
cargo build --release

echo "==> cargo clippy (bare-metal)"
cargo clippy -- -D warnings

echo "==> cargo clippy (host tooling)"
cargo clippy -p rawboot-image -p xtask --target "$host_target" --all-targets -- -D warnings

echo "==> cargo test (host)"
cargo test -p rawboot-image -p xtask --target "$host_target"

echo "==> cargo xtask image"
cargo xtask image

echo "==> cargo xtask payload"
cargo xtask payload

echo "all checks passed"
