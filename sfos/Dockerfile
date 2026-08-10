# Reproducible Sailfish OS build image for KeePassRX.
#
# Modeled on Whisperfish's sailo-rs `Dockerfile.rust`: it starts from the
# CODeRUS dockerized Sailfish Platform SDK and installs a modern Rust toolchain
# (the SDK ships only Rust 1.75, but KeePassRX is edition 2024 and needs >=1.85)
# plus the app's build dependencies, into BOTH the shared tooling and the
# per-arch target (Rust must be present in both for SB2 cross builds).
#
# The newer Rust/LLVM come from the community "rubdos" repo
# (https://nas.rubdos.be/~rsmet/sailfish-repo/), the same source Whisperfish
# uses. NOTE: this repo is unsigned and is a third party's personal server.
#
# Build the image (from the repo root):
#   docker build -f sfos/Dockerfile \
#       --build-arg SFOS_VERSION=5.1.0.11 \
#       --build-arg SFOS_ARCH=aarch64 \
#       --build-arg RUST_TARGET=aarch64-unknown-linux-gnu \
#       -t keepassrx-sfos-build:aarch64 .
#   (emulator: SFOS_ARCH=i486  RUST_TARGET=i686-unknown-linux-gnu)
#
# Then build the RPM with sfos/build.sh (see that script).

ARG SFOS_VERSION=5.1.0.11
ARG SFOS_ARCH=aarch64
FROM coderus/sailfishos-platform-sdk-$SFOS_ARCH:$SFOS_VERSION

ARG SFOS_VERSION=5.1.0.11
ARG SFOS_ARCH=aarch64
ARG RUST_TARGET=aarch64-unknown-linux-gnu

ARG MINIMUM_RUST_VERSION=1.89
ARG MINIMUM_LLVM_VERSION=20
ARG RUBDOS_REPO=https://nas.rubdos.be/~rsmet/sailfish-repo/

USER mersdk

RUN set -eux; \
    echo "Adding rubdos repo (unsigned) for a newer Rust..."; \
    sdk-manage tooling maintain SailfishOS-$SFOS_VERSION \
        zypper --non-interactive addrepo --gpgcheck-allow-unsigned $RUBDOS_REPO rubdos || true; \
    sdk-manage target maintain SailfishOS-$SFOS_VERSION-$SFOS_ARCH \
        zypper --non-interactive addrepo --gpgcheck-allow-unsigned $RUBDOS_REPO rubdos || true; \
    \
    echo "Installing modern Rust + LLVM into the tooling..."; \
    sdk-manage tooling maintain SailfishOS-$SFOS_VERSION \
        zypper --non-interactive --gpg-auto-import-keys refresh; \
    sdk-manage tooling maintain SailfishOS-$SFOS_VERSION \
        zypper --non-interactive install --allow-vendor-change --from rubdos \
            "rust >= $MINIMUM_RUST_VERSION" cargo \
            rust-std-static-aarch64-unknown-linux-gnu \
            rust-std-static-armv7-unknown-linux-gnueabihf \
            rust-std-static-i686-unknown-linux-gnu \
            "llvm >= $MINIMUM_LLVM_VERSION" "clang >= $MINIMUM_LLVM_VERSION"; \
    \
    echo "Installing modern Rust + build deps into the $SFOS_ARCH target..."; \
    sdk-manage target maintain SailfishOS-$SFOS_VERSION-$SFOS_ARCH \
        zypper --non-interactive --gpg-auto-import-keys refresh; \
    if [ "$SFOS_ARCH" = "i486" ]; then \
        sdk-manage target maintain SailfishOS-$SFOS_VERSION-$SFOS_ARCH \
            zypper --non-interactive install --allow-vendor-change --from rubdos \
                "rust >= $MINIMUM_RUST_VERSION" cargo \
                rust-std-static-i686-unknown-linux-gnu \
                "llvm >= $MINIMUM_LLVM_VERSION" "clang >= $MINIMUM_LLVM_VERSION"; \
    elif [ "$SFOS_ARCH" = "armv7hl" ]; then \
        sdk-manage target maintain SailfishOS-$SFOS_VERSION-$SFOS_ARCH \
            zypper --non-interactive install --allow-vendor-change --from rubdos \
                "rust >= $MINIMUM_RUST_VERSION" cargo \
                rust-std-static-armv7-unknown-linux-gnueabihf \
                rust-std-static-i686-unknown-linux-gnu \
                "llvm >= $MINIMUM_LLVM_VERSION" "clang >= $MINIMUM_LLVM_VERSION"; \
    else \
        sdk-manage target maintain SailfishOS-$SFOS_VERSION-$SFOS_ARCH \
            zypper --non-interactive install --allow-vendor-change --from rubdos \
                "rust >= $MINIMUM_RUST_VERSION" cargo \
                rust-std-static-aarch64-unknown-linux-gnu \
                rust-std-static-i686-unknown-linux-gnu \
                "llvm >= $MINIMUM_LLVM_VERSION" "clang >= $MINIMUM_LLVM_VERSION"; \
    fi; \
    \
    echo "Installing KeePassRX build dependencies into the target..."; \
    sdk-manage target maintain SailfishOS-$SFOS_VERSION-$SFOS_ARCH \
        zypper --non-interactive install \
            libatomic libatomic-static \
            gettext desktop-file-utils \
            tar make gzip xz \
            pkgconfig(sailfishapp) \
            pkgconfig(Qt5Core) pkgconfig(Qt5Qml) pkgconfig(Qt5Quick) pkgconfig(Qt5Gui); \
    \
    echo "Verifying cross Rust..."; \
    sb2 -t SailfishOS-$SFOS_VERSION-$SFOS_ARCH rustc --version; \
    sudo rm -rf /srv/mer/targets/SailfishOS-$SFOS_VERSION-$SFOS_ARCH/var/cache/zypp/* || true
