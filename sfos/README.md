# Sailfish OS build

The Ubuntu Touch build is handled by clickable (`clickable.yaml`). This is the
Sailfish side.

SFOS SDK targets ship Rust 1.75 and this crate is edition 2024, so the stock
toolchain won't build it. Rather than provision a newer Rust ourselves, we build
on top of Whisperfish's `sailo-rs` image, which already carries a working Rust
1.89 for the Sailfish SDK. That image is published for SFOS 5.0.0.43 only, so
that's the version we target (its RPMs run on newer devices too).

## Build

The `sfos/Dockerfile` image does the whole build (its entrypoint is
`docker-entrypoint.sh`): mount the source at `/src`, the output at `/out`, run
it. For one arch that's a single command:

    docker run --rm -v "$PWD":/src:ro -v "$PWD/RPMS":/out \
      keepassrx-sfos-build:5.0.0.43-aarch64

`sfos/build.sh` wraps that — it builds the per-arch image on first use, computes
the git version, and can do several arches at once:

    sfos/build.sh              # emulator (i486)
    sfos/build.sh aarch64      # arm64 device
    sfos/build.sh i486 aarch64 # both
    sfos/build.sh --stable ... # spec version, not the git-derived one

RPMs land in `RPMS/`, one per arch — building a second arch (or rebuilding one)
preserves the others. Env: `SFOS_VERSION`, `DOCKER`, `REBUILD_IMAGE=1`.

Building on a bind-mount/volume breaks scratchbox2, so the entrypoint copies the
source into the container and builds there; the cargo target dir is cached in a
`/cache` volume per arch (mount one with `-v keepassrx-sfos-cache:/cache` for a
standalone run). First build is slow — it cross-compiles the whole dependency
tree under SB2, including libsodium from source.

## Layout

`rpm/harbour-keepassrx.spec` does the build: cross-compile `keepassrx` with
`--features sailfish`, then install the binary, the Silica QML from
`keepass-rx/qml-sfos/`, the shared assets, the gettext catalogs, and the desktop
file.
