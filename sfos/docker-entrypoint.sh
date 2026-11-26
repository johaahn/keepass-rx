#!/bin/bash
# Build RPM(s) from source at /src into /out. Args: ARCH... (default: image arch).
set -euo pipefail

: "${SFOS_VERSION:?}"
arches=("$@")
[ ${#arches[@]} -gt 0 ] || arches=("${SFOS_ARCH:?}")

define=()
[ -z "${KEEPASSRX_VERSION:-}" ] || define=(-- --define "keepassrx_version $KEEPASSRX_VERSION")

for arch in "${arches[@]}"; do
    b="/home/mersdk/build-$arch"
    mkdir -p "$b"
    sudo chown "$(id -u):$(id -g)" "$b"
    find "$b" -mindepth 1 -maxdepth 1 ! -name target -exec rm -rf -- {} +

    if [ -d /cache ]; then
        sudo chown -R "$(id -u):$(id -g)" /cache
        if [ -d "/cache/$arch" ]; then rm -rf "$b/target"; mv "/cache/$arch" "$b/target"; fi
    fi
    mkdir -p "$b/target"
    sudo chown -R "$(id -u):$(id -g)" "$b/target"
    [ ! -d "$HOME/.cargo" ] || sudo chown -R "$(id -u):$(id -g)" "$HOME/.cargo"

    tar -C /src --exclude=./.git --exclude=./target --exclude=./build \
        --exclude=./RPMS --exclude=./.clickable -cf - . | tar -C "$b" -xf -

    mkdir -p "$b/.sfdk"
    cp "$b/rpm/harbour-keepassrx.spec" "$b/.sfdk/spec"
    printf 'SailfishOS-%s-%s.default\n' "$SFOS_VERSION" "$arch" > "$b/.sfdk/target"
    printf '*\n' > "$b/.sfdk/.gitignore"

    ( cd "$b" && mb2 -t "SailfishOS-$SFOS_VERSION-$arch" --no-snapshot=force build --no-check "${define[@]}" )

    shopt -s nullglob
    rpms=("$b"/RPMS/*.rpm)
    shopt -u nullglob
    [ ${#rpms[@]} -gt 0 ] || { echo "No RPM produced for $arch" >&2; exit 1; }
    if [ -d /out ]; then
        sudo chmod 0777 /out
        for rpm in "${rpms[@]}"; do
            rpm -qpl "$rpm" >/dev/null
            cp -f "$rpm" "/out/$(basename "$rpm")"
        done
    fi
    if [ -d /cache ]; then rm -rf "/cache/$arch"; mv "$b/target" "/cache/$arch"; fi
done
