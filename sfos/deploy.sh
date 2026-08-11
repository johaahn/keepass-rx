#!/usr/bin/env bash
#
# Install the KeePassRX RPM onto a running Sailfish OS emulator.
#
#   sfos/deploy.sh            # use newest built RPM, build it if none exists
#   sfos/deploy.sh --build    # rebuild first, then deploy
#
# Connection details (ssh port, user, key) are discovered from
# `sfdk device list`, so nothing here is machine-specific. The emulator must be
# running (start it from the SDK / Qt Creator, or `sfdk emulator start <name>`).
#
# Env: SFDK (path to sfdk), SFOS_VERSION, ARCH (default i486, the emulator arch).
set -euo pipefail

SFDK="${SFDK:-$(command -v sfdk || echo "$HOME/SailfishOS/bin/sfdk")}"
[ -x "$SFDK" ] || { echo "sfdk not found (set \$SFDK)" >&2; exit 1; }

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ARCH="${ARCH:-i486}"

# --- Locate (or build) the RPM ---
find_rpm() { ls -t "$REPO_ROOT"/RPMS/harbour-keepassrx-*."$ARCH".rpm 2>/dev/null | head -1; }
rpm="$(find_rpm || true)"
if [ "${1:-}" = "--build" ] || [ -z "$rpm" ]; then
    echo ">> Building $ARCH RPM"
    "$REPO_ROOT/sfos/build.sh" "$ARCH"
    rpm="$(find_rpm || true)"
fi
[ -n "$rpm" ] || { echo "No $ARCH RPM in RPMS/ (run sfos/build.sh $ARCH)" >&2; exit 1; }
echo ">> RPM: $(basename "$rpm")"

# --- Discover the running emulator's ssh connection ---
devlist="$("$SFDK" device list 2>/dev/null || true)"
conn="$(printf '%s\n' "$devlist" | grep -oE '[a-zA-Z0-9_]+@[0-9.]+:[0-9]+' | head -1)"
key="$(printf '%s\n' "$devlist" | sed -nE 's/.*private-key:[[:space:]]*//p' | head -1)"
key="${key/#\~/$HOME}"
[ -n "$conn" ] || { echo "No emulator/device found in 'sfdk device list'. Is the emulator running?" >&2; exit 1; }
[ -n "$key" ] || { echo "Could not find the ssh private key in 'sfdk device list'" >&2; exit 1; }

user="${conn%@*}"; rest="${conn#*@}"; host="${rest%:*}"; port="${rest#*:}"
echo ">> Emulator: $conn (key: $key)"

sshopts=(-i "$key" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR)
base="$(basename "$rpm")"

# --- Copy + install (root via passwordless sudo of the sdk user) ---
scp -P "$port" "${sshopts[@]}" "$rpm" "$user@$host:/tmp/$base"
ssh -p "$port" "${sshopts[@]}" "$user@$host" \
    "sudo pkcon install-local -y /tmp/$base && (pkill -x harbour-keepassrx 2>/dev/null || true) && rpm -q harbour-keepassrx"

echo ">> Installed. Launch KeePassRX from the emulator's app grid."
