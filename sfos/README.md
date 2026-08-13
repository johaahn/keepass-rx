# Sailfish OS build

The Ubuntu Touch build is handled by clickable (`clickable.yaml`). This is the
Sailfish side.

SFOS SDK targets ship Rust 1.75 and this crate is edition 2024, so the stock
toolchain won't build it. There's no official newer Rust for Sailfish, so we do
what Whisperfish does: pull Rust 1.89 plus LLVM/clang from the rubdos repo
(https://nas.rubdos.be/~rsmet/sailfish-repo/). It's an unsigned repo run by the
Whisperfish maintainer, which is worth knowing before you add it.

## Build

With the Jolla SDK installed, from the repo root:

    sfos/build.sh              # emulator (i486)
    sfos/build.sh aarch64      # arm64 device
    sfos/build.sh i486 aarch64 # both

That's the whole thing. The script provisions the toolings/targets (adds the
repo, installs Rust and the build deps) the first time and then builds; on later
runs it sees the toolchain is already there and goes straight to building. RPMs
land in `RPMS/`, one per arch — building a second arch (or rebuilding one)
preserves the others' packages. Set `SFOS_VERSION` for a different release
(default 5.1.0.11).

By default sfdk derives the package version from git (e.g.
`1.0.0+sailfish.<timestamp>.<hash>`). Pass `--stable` to use the plain `Version:`
from the spec instead, for release builds:

    sfos/build.sh --stable aarch64

The repo has to sit under the SDK's shared workspace (the home directory by
default) or sfdk can't see it.

First build is slow — it cross-compiles the whole dependency tree under SB2,
including libsodium from source. Each arch caches into its own
`target/sfos-<triple>/`, so subsequent builds (and switching between arches) are
much faster. Cargo is capped to 4 jobs (`CARGO_BUILD_JOBS` in the spec) because
scratchbox2 deadlocks at full parallelism; raise it with
`--define "cargo_jobs N"` if your setup tolerates it.

Don't `kill -9` a build mid-flight — that can orphan a cargo process holding
`~/.cargo/.package-cache` in the engine and wedge every later build. build.sh
clears a stale lock when nothing is building; if you're stuck, kill the engine's
cargo/rpmbuild tree and `rm` that file.

## CI / clean room

`Dockerfile` bakes the same environment on top of the CODeRUS platform SDK image
for CI, where you don't have a persistent SDK:

    docker build -f sfos/Dockerfile \
      --build-arg SFOS_VERSION=5.1.0.11 \
      --build-arg SFOS_ARCH=aarch64 \
      --build-arg RUST_TARGET=aarch64-unknown-linux-gnu \
      -t keepassrx-sfos:aarch64 .
    docker run --rm -v "$PWD":/build -w /build keepassrx-sfos:aarch64 \
      mb2 -t SailfishOS-5.1.0.11-aarch64 build

## Layout

`rpm/harbour-keepassrx.spec` does the build: cross-compile `keepassrx` with
`--features sailfish`, then install the binary, the Silica QML from
`keepass-rx/qml-sfos/`, the shared assets, the gettext catalogs, and the desktop
file.
