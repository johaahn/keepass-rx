#!/usr/bin/env bash
#
# Convenience wrapper around the sfos/Dockerfile image (which does the build).
# Equivalent one-liner for a single arch:
#   docker run --rm -v "$PWD":/src:ro -v "$PWD/RPMS":/out \
#       keepassrx-sfos-build:5.0.0.43-aarch64
#
#   sfos/build.sh              # emulator (i486)
#   sfos/build.sh aarch64      # arm64 device
#   sfos/build.sh i486 aarch64 # both
#   sfos/build.sh --stable ... # spec version, not the git-derived one
#
# Env: SFOS_VERSION (default 5.0.0.43), DOCKER, REBUILD_IMAGE. RPMs -> RPMS/.
set -euo pipefail

SFOS_VERSION="${SFOS_VERSION:-5.0.0.43}"

DOCKER="${DOCKER:-$(command -v docker || command -v podman || true)}"
[ -n "$DOCKER" ] || { echo "no container runtime (install docker/podman or set \$DOCKER)" >&2; exit 1; }

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

ARCHES=()
stable=
for arg in "$@"; do
    case "$arg" in
        --stable) stable=1 ;;
        -*) echo "unknown option: $arg" >&2; exit 1 ;;
        *) ARCHES+=("$arg") ;;
    esac
done
[ ${#ARCHES[@]} -gt 0 ] || ARCHES=(i486)

version_env=()
if [ -z "$stable" ]; then
    desc="$(git -C "$REPO_ROOT" describe --tags --always --dirty 2>/dev/null | sed -e 's/^v//' -e 's/[^A-Za-z0-9.]/./g' || true)"
    [ -n "$desc" ] && version_env=(-e "KEEPASSRX_VERSION=$desc")
fi

supported() { case "$1" in i486|armv7hl|aarch64) return 0 ;; *) echo "unsupported arch: $1" >&2; return 1 ;; esac; }

ensure_image() { # $1 = arch, $2 = image tag
    if [ -z "${REBUILD_IMAGE:-}" ] && $DOCKER image inspect "$2" >/dev/null 2>&1; then
        return
    fi
    echo ">> Building image $2"
    $DOCKER build -f "$REPO_ROOT/sfos/Dockerfile" -t "$2" \
        --build-arg SFOS_VERSION="$SFOS_VERSION" --build-arg SFOS_ARCH="$1" "$REPO_ROOT/sfos"
}

mkdir -p "$REPO_ROOT/RPMS"
shopt -s nullglob

for arch in "${ARCHES[@]}"; do
    supported "$arch"
    image="keepassrx-sfos-build:$SFOS_VERSION-$arch"
    ensure_image "$arch" "$image"
    echo ">> Building for SailfishOS-$SFOS_VERSION-$arch"
    old=("$REPO_ROOT"/RPMS/*."$arch".rpm)
    $DOCKER run --rm \
        -v "$REPO_ROOT":/src:ro \
        -v "keepassrx-sfos-cache-$SFOS_VERSION":/cache \
        -v "$REPO_ROOT/RPMS":/out \
        "${version_env[@]}" "$image"
    new=("$REPO_ROOT"/RPMS/*."$arch".rpm)
    if [ ${#old[@]} -gt 0 ] && [ ${#new[@]} -gt ${#old[@]} ]; then
        rm -f "${old[@]}"
    fi
done

echo ">> RPMs:"
ls -1 "$REPO_ROOT"/RPMS/*.rpm 2>/dev/null || echo "(none found in RPMS/)"
