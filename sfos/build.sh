#!/usr/bin/env bash
#
# Build the KeePassRX Sailfish OS RPM(s) with the Jolla SDK (sfdk), from a fresh
# SDK install and with no manual setup: this provisions the toolings/targets
# with a modern Rust toolchain (see sfos/README.md for why) and then builds.
#
#   sfos/build.sh              # emulator (i486)
#   sfos/build.sh aarch64      # arm64 device
#   sfos/build.sh i486 aarch64 # both
#
# Env:
#   SFOS_VERSION   SDK release to build against (default 5.1.0.11)
#   SFDK           path to sfdk (default: sfdk on PATH, else ~/SailfishOS/bin/sfdk)
#
# RPMs are written to RPMS/.
set -euo pipefail

SFOS_VERSION="${SFOS_VERSION:-5.1.0.11}"
RUBDOS_REPO=https://nas.rubdos.be/~rsmet/sailfish-repo/
MIN_RUST=1.89
MIN_LLVM=20

SFDK="${SFDK:-$(command -v sfdk || echo "$HOME/SailfishOS/bin/sfdk")}"
[ -x "$SFDK" ] || { echo "sfdk not found (set \$SFDK)" >&2; exit 1; }

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

ARCHES=("$@")
[ ${#ARCHES[@]} -gt 0 ] || ARCHES=(i486)

tooling="SailfishOS-$SFOS_VERSION"

# std packages needed in the tooling (all arches) and per target.
all_std="rust-std-static-i686-unknown-linux-gnu rust-std-static-armv7-unknown-linux-gnueabihf rust-std-static-aarch64-unknown-linux-gnu"
std_for() {
    case "$1" in
        SailfishOS-*-i486)    echo "rust-std-static-i686-unknown-linux-gnu" ;;
        SailfishOS-*-armv7hl) echo "rust-std-static-armv7-unknown-linux-gnueabihf rust-std-static-i686-unknown-linux-gnu" ;;
        SailfishOS-*-aarch64) echo "rust-std-static-aarch64-unknown-linux-gnu rust-std-static-i686-unknown-linux-gnu" ;;
    esac
}

# True if the given tooling/target already has a new-enough Rust.
rust_ok() { # $1 = maintain scope (e.g. "target maintain SailfishOS-...-i486")
    local v
    v=$($SFDK engine exec -- sdk-manage $1 rustc --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+' | head -1) || return 1
    [ -n "$v" ] && [ "$(printf '%s\n%s\n' "$MIN_RUST" "$v" | sort -V | head -1)" = "$MIN_RUST" ]
}

add_repo() { # $1 = maintain scope
    $SFDK engine exec -- sudo sdk-manage $1 zypper -n addrepo --gpgcheck-allow-unsigned "$RUBDOS_REPO" rubdos >/dev/null 2>&1 || true
    $SFDK engine exec -- sudo sdk-manage $1 zypper -n --gpg-auto-import-keys refresh >/dev/null
}

echo ">> Ensuring build engine is running"
$SFDK engine start >/dev/null

# --- Provision the tooling once (shared by all targets) ---
if rust_ok "tooling maintain $tooling"; then
    echo ">> Tooling $tooling already has Rust >= $MIN_RUST"
else
    echo ">> Provisioning tooling $tooling"
    add_repo "tooling maintain $tooling"
    $SFDK engine exec -- sudo sdk-manage tooling maintain "$tooling" \
        zypper -n install --allow-vendor-change --from rubdos \
        "rust >= $MIN_RUST" cargo $all_std "llvm >= $MIN_LLVM" "clang >= $MIN_LLVM"
fi

# --- Provision each requested target ---
for arch in "${ARCHES[@]}"; do
    target="SailfishOS-$SFOS_VERSION-$arch"
    if rust_ok "target maintain $target"; then
        echo ">> Target $target already has Rust >= $MIN_RUST"
    else
        echo ">> Provisioning target $target"
        add_repo "target maintain $target"
        $SFDK engine exec -- sudo sdk-manage target maintain "$target" \
            zypper -n install --allow-vendor-change --from rubdos \
            "rust >= $MIN_RUST" cargo $(std_for "$target") "llvm >= $MIN_LLVM" "clang >= $MIN_LLVM"
    fi
    # Build-time deps (idempotent; no-ops once present).
    $SFDK engine exec -- sudo sdk-manage target maintain "$target" \
        zypper -n install \
        libatomic libatomic-static gettext desktop-file-utils tar make gzip xz \
        'pkgconfig(sailfishapp)' 'pkgconfig(Qt5Core)' 'pkgconfig(Qt5Qml)' \
        'pkgconfig(Qt5Quick)' 'pkgconfig(Qt5Gui)' >/dev/null
done

# A build that was hard-killed can leave an orphaned cargo/rustc holding cargo's
# global package-cache lock, which then deadlocks every later build. Clear a
# stale lock when nothing is actually building.
if [ "$($SFDK engine exec -- sh -c 'ps -eo args 2>/dev/null | grep -c "[c]argo build --release"' 2>/dev/null || echo 0)" -eq 0 ]; then
    $SFDK engine exec -- sh -c 'rm -f /home/mersdk/.cargo/.package-cache' >/dev/null 2>&1 || true
fi

# --- Build ---
# mb2 clears RPMS/ at the start of each build, so building a second arch would
# delete the first arch's package. Stash each arch's output and restore at the
# end so all requested arches survive.
stash="$(mktemp -d)"
for arch in "${ARCHES[@]}"; do
    target="SailfishOS-$SFOS_VERSION-$arch"
    echo ">> Building for $target"
    $SFDK -c target="$target" build
    cp -f "$REPO_ROOT"/RPMS/*."$arch".rpm "$stash"/ 2>/dev/null || true
done
mkdir -p "$REPO_ROOT/RPMS"
cp -f "$stash"/*.rpm "$REPO_ROOT/RPMS"/ 2>/dev/null || true
rm -rf "$stash"

echo ">> RPMs:"
ls -1 "$REPO_ROOT"/RPMS/*.rpm 2>/dev/null || echo "(none found in RPMS/)"
