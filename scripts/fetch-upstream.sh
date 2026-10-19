#!/usr/bin/env bash
# SPDX-License-Identifier: Unlicense
#
# Fetch a pinned szemzoa/awboot tree for parity work. Defaults to the commit the
# current ADR/research notes are anchored to. The tree lands under target/, which
# is git-ignored.
set -euo pipefail

cd "$(dirname "$0")/.."

SHA=${1:-5380c00fc67c975433f25c57fb481aa2b91aebf8}
DEST=${2:-target/upstream/awboot-${SHA}}

if [ -d "$DEST" ]; then
    echo "upstream $SHA already present at $DEST"
    exit 0
fi

mkdir -p "$(dirname "$DEST")"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

url="https://github.com/szemzoa/awboot/archive/${SHA}.tar.gz"
echo "fetching $url"
curl -fsSL "$url" -o "$tmp/awboot.tar.gz"
tar -xzf "$tmp/awboot.tar.gz" -C "$tmp"

src="$tmp/awboot-$SHA"
if [ ! -d "$src" ]; then
    echo "error: archive did not contain awboot-$SHA" >&2
    exit 1
fi

mv "$src" "$DEST"
echo "upstream $SHA -> $DEST"
