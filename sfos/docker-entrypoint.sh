#!/bin/bash
# Build RPM(s) from source at /src into /out. Args: ARCH... (default: image arch).
set -euo pipefail

: "${SFOS_VERSION:?}"
arches=("$@")
[ ${#arches[@]} -gt 0 ] || arches=("${SFOS_ARCH:?}")

define=()
[ -n "${KEEPASSRX_VERSION:-}" ] && define=(-- --define "keepassrx_version $KEEPASSRX_VERSION")

for arch in "${arches[@]}"; do
    b="/home/mersdk/build-$arch"
    rm -rf "$b"; mkdir -p "$b"
    tar -C /src --exclude=./.git --exclude=./target --exclude=./build \
        --exclude=./RPMS --exclude=./.clickable -cf - . | tar -C "$b" -xf -

    if [ -d /cache ]; then
        sudo chown -R "$(id -u):$(id -g)" /cache
        if [ -d "/cache/$arch" ]; then mv "/cache/$arch" "$b/target"; fi
    fi

    ( cd "$b" && mb2 -t "SailfishOS-$SFOS_VERSION-$arch" --no-snapshot=force build --no-check "${define[@]}" )

    if [ -d /out ]; then
        sudo chmod 0777 /out
        cp -f "$b"/RPMS/*.rpm /out/
    fi
    if [ -d /cache ] && [ -d "$b/target" ]; then rm -rf "/cache/$arch"; mv "$b/target" "/cache/$arch"; fi
done
