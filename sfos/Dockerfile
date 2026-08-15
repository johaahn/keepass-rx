# Sailfish OS build image for KeePassRX, on Whisperfish's sailo-rs image
# (a working Rust 1.89 for the SDK). sailo-rs publishes 5.0.0.43 only.
#
#   docker build -f sfos/Dockerfile \
#       --build-arg SFOS_VERSION=5.0.0.43 --build-arg SFOS_ARCH=aarch64 \
#       -t keepassrx-sfos-build:5.0.0.43-aarch64 sfos

ARG SFOS_VERSION=5.0.0.43
ARG SFOS_ARCH=aarch64
FROM registry.gitlab.com/whisperfish/sailo-rs/rust-${SFOS_ARCH}-${SFOS_VERSION}:latest

ARG SFOS_VERSION=5.0.0.43
ARG SFOS_ARCH=aarch64
ARG CARGO_ABOUT_VERSION=0.8.2

USER mersdk

RUN sdk-manage target maintain SailfishOS-$SFOS_VERSION-$SFOS_ARCH \
        zypper --non-interactive install \
            libatomic libatomic-static gettext desktop-file-utils \
            tar make gzip xz \
            'pkgconfig(sailfishapp)' 'pkgconfig(Qt5Core)' 'pkgconfig(Qt5Qml)' \
            'pkgconfig(Qt5Quick)' 'pkgconfig(Qt5Gui)'

# No i686 cargo-about prebuilt: build in the tooling, place in the target sysroot.
RUN sdk-manage tooling maintain SailfishOS-$SFOS_VERSION \
        cargo install --locked --root /usr --version $CARGO_ABOUT_VERSION cargo-about \
 && cp /srv/mer/toolings/SailfishOS-$SFOS_VERSION/usr/bin/cargo-about \
       /srv/mer/targets/SailfishOS-$SFOS_VERSION-$SFOS_ARCH/usr/bin/cargo-about

ENV SFOS_VERSION=$SFOS_VERSION SFOS_ARCH=$SFOS_ARCH
COPY --chmod=0755 docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
