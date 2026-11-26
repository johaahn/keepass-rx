#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
[ -f robots/rust/check.py ] || echo 'check: robots/ is missing; run git submodule update --init' >&2
exec python3 -I robots/rust/check.py "$@"
