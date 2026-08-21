# Sailfish OS RPM spec for KeePassRX.
#
# Build with the Sailfish SDK, e.g. from the repository root:
#   sfdk build            # or: mb2 -t <target> build
#
# The Ubuntu Touch build is unaffected by this file; it is produced by
# clickable from clickable.yaml.

%bcond_with harbour

# Rust cross-compile target triple, chosen from the RPM build arch. Required
# because rustc under the Sailfish SB2 target otherwise defaults to the host.
%ifarch %arm
%define rust_triple armv7-unknown-linux-gnueabihf
%endif
%ifarch aarch64
%define rust_triple aarch64-unknown-linux-gnu
%endif
%ifarch %ix86
%define rust_triple i686-unknown-linux-gnu
%endif

%define targetdir target/sfos-%{rust_triple}/%{rust_triple}/release
%define datadir %{_datadir}/%{name}

Name:       harbour-keepassrx
Summary:    Password manager compatible with KeePass databases
Version:    %{?keepassrx_version}%{!?keepassrx_version:1.0.1}
Release:    1
License:    AGPLv3
Group:      Qt/Qt
URL:        https://github.com/projectmoon/keepass-rx
Source0:    %{name}-%{version}.tar.bz2

Requires:   sailfishsilica-qt5 >= 0.10.9
Requires:   nemo-qml-plugin-configuration-qt5
Requires:   sailfish-components-pickers-qt5

BuildRequires:  pkgconfig(sailfishapp) >= 1.0.3
BuildRequires:  pkgconfig(Qt5Core)
BuildRequires:  pkgconfig(Qt5Qml)
BuildRequires:  pkgconfig(Qt5Quick)
BuildRequires:  pkgconfig(Qt5Gui)
BuildRequires:  rust
BuildRequires:  rust-std-static
BuildRequires:  cargo
BuildRequires:  gettext
BuildRequires:  libatomic-static
BuildRequires:  desktop-file-utils

%description
KeePassRX is a native Sailfish OS password manager compatible with KeePass
(.kdbx / .kdb) databases. Read-only access to entries, groups, TOTP/Steam
codes, key files and attachments.

%prep
%setup -q -n %{name}-%{version}

%build
rustc --version
cargo --version

%ifarch %arm
export SB2_RUST_TARGET_TRIPLE=armv7-unknown-linux-gnueabihf
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=armv7hl-meego-linux-gnueabi-gcc
export CC_armv7_unknown_linux_gnueabihf=armv7hl-meego-linux-gnueabi-gcc
export CXX_armv7_unknown_linux_gnueabihf=armv7hl-meego-linux-gnueabi-g++
export AR_armv7_unknown_linux_gnueabihf=armv7hl-meego-linux-gnueabi-ar
%endif
%ifarch aarch64
export SB2_RUST_TARGET_TRIPLE=aarch64-unknown-linux-gnu
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-meego-linux-gnu-gcc
export CC_aarch64_unknown_linux_gnu=aarch64-meego-linux-gnu-gcc
export CXX_aarch64_unknown_linux_gnu=aarch64-meego-linux-gnu-g++
export AR_aarch64_unknown_linux_gnu=aarch64-meego-linux-gnu-ar
export CFLAGS_aarch64_unknown_linux_gnu="$CFLAGS -march=armv8-a+crypto+fp+simd+sha2"
%endif
%ifarch %ix86
export SB2_RUST_TARGET_TRIPLE=i686-unknown-linux-gnu
%endif

# Per-arch target dir so alternating arches don't invalidate each other's
# shared host build cache (cascades into a full rebuild). Under target/ (gitignored).
export CARGO_TARGET_DIR=target/sfos-%{rust_triple}

# qttypes' build script tries qmake6 first; Sailfish only ships the Qt5 qmake.
export QMAKE=/usr/bin/qmake

# rpmbuild hands build scripts a sanitized PATH; under SB2 that omits the dir
# holding tar/make, so source-building crates (libsodium) fail to find them.
export PATH=/usr/bin:/bin:/usr/sbin:/sbin:$PATH

# Use the system gettext (libintl) rather than compiling GNU gettext from source
# during the build (which needs tar/configure and is slow). The `gettext`
# package is a BuildRequires.
export GETTEXT_SYSTEM=1

# Cap cargo's parallelism. Under parallel rustc, scratchbox2's fork/exec
# emulation intermittently fails to reap child processes, leaving zombies and
# deadlocking the build (most reproducible on the cross targets). Only a single
# job reliably avoids the race, so build serially by default. Override with
# --define "cargo_jobs N".
export CARGO_BUILD_JOBS=%{?cargo_jobs}%{!?cargo_jobs:1}

export PKG_CONFIG_ALLOW_CROSS_i686_unknown_linux_gnu=1
export PKG_CONFIG_ALLOW_CROSS_armv7_unknown_linux_gnueabihf=1
export PKG_CONFIG_ALLOW_CROSS_aarch64_unknown_linux_gnu=1

# RUSTFLAGS, when set, fully replaces the rustflags in .cargo/config.toml. We
# set it explicitly for every arch so the host-only `-C target-cpu=native` from
# that file never leaks into the cross build, and re-add the aarch64 crypto
# features (aes_armv8) here where they belong.
export RUSTFLAGS=""
%ifarch aarch64
export RUSTFLAGS="$RUSTFLAGS -C target-feature=+sha2,+neon --cfg aes_armv8"
%endif

%if %{with harbour}
FEATURES="sailfish,harbour"
%else
FEATURES="sailfish"
%endif

cargo build \
    --release \
    --no-default-features \
    -p keepassrx \
    --bin keepassrx \
    --features $FEATURES

%install
rm -rf %{buildroot}

# Application binary.
install -Dm 755 %{targetdir}/keepassrx %{buildroot}%{_bindir}/%{name}

# Silica QML tree (loaded at runtime via SailfishApp::pathTo, i.e. relative to
# %{_datadir}/%{name}).
find keepass-rx/qml-sfos -type f | while read -r f; do
    rel=${f#keepass-rx/qml-sfos/}
    install -Dm 644 "$f" "%{buildroot}%{datadir}/qml/$rel"
done

# Shared assets (icons, images) used by both platforms.
find keepass-rx/assets -type f | while read -r f; do
    rel=${f#keepass-rx/assets/}
    install -Dm 644 "$f" "%{buildroot}%{datadir}/assets/$rel"
done

# Compiled gettext catalogs (build.rs skips these outside clickable).
for po in po/*.po; do
    [ -e "$po" ] || continue
    lang=$(basename "$po" .po)
    install -d "%{buildroot}%{_datadir}/locale/$lang/LC_MESSAGES"
    msgfmt "$po" -o "%{buildroot}%{_datadir}/locale/$lang/LC_MESSAGES/keepassrx.projectmoon.mo"
done

# Desktop launcher + icon. Installed verbatim so the [X-Sailjail] section is
# preserved (desktop-file-install strips unknown groups).
install -Dm 644 rpm/%{name}.desktop \
    %{buildroot}%{_datadir}/applications/%{name}.desktop
install -Dm 644 keepass-rx/assets/logo-sfos.png \
    %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/%{name}.png

%files
%defattr(-,root,root,-)
%{_bindir}/%{name}
%{datadir}
%{_datadir}/applications/%{name}.desktop
%{_datadir}/icons/hicolor/128x128/apps/%{name}.png
%{_datadir}/locale/*/LC_MESSAGES/keepassrx.projectmoon.mo

%changelog
* Sun Aug 09 2026 projectmoon <projectmoon@agnos.is> - 1.0.1-1
- Initial Sailfish OS port.
