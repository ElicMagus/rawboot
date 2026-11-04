#!/usr/bin/env bash
# SPDX-License-Identifier: CC-BY-4.0
#
# Guard the loader's read-only dependency graph. `rawboot-flasher` enables
# sunxi-drivers/write, so building it as a default member would unify that
# feature into `rawboot` and blow its SRAM budget. Run this before a build.
set -euo pipefail

cd "$(dirname "$0")/.."

if cargo tree -p rawboot -e features | grep -F 'sunxi-drivers feature "write"' >/dev/null; then
    echo "error: rawboot resolved sunxi-drivers/write; the loader must build without it" >&2
    echo "       keep rawboot-flasher out of default-members (ADR-0014)" >&2
    exit 1
fi
