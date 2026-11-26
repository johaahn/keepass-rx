#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${1:-}"

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "usage: $0 MAJOR.MINOR.PATCH" >&2
    exit 1
fi

sed -i "0,/^version = \".*\"/s//version = \"$VERSION\"/" "$ROOT/keepass-rx/Cargo.toml"
sed -i "/^name = \"keepassrx\"$/{n;s/^version = \".*\"/version = \"$VERSION\"/}" "$ROOT/Cargo.lock"
sed -i "s/^\(    \"version\": \)\"[^\"]*\"/\1\"$VERSION\"/" "$ROOT/manifest.json"
sed -i "s/%{!?keepassrx_version:[^}]*}/%{!?keepassrx_version:$VERSION}/" "$ROOT/rpm/harbour-keepassrx.spec"

grep -H -m1 '^version = ' "$ROOT/keepass-rx/Cargo.toml"
grep -H -A1 '^name = "keepassrx"$' "$ROOT/Cargo.lock" | tail -1
grep -H '"version"' "$ROOT/manifest.json"
grep -H '^Version:' "$ROOT/rpm/harbour-keepassrx.spec"
