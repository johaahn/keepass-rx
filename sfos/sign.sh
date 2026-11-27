#!/usr/bin/env bash
set -euo pipefail

SIGN_IMAGE="${SIGN_IMAGE:-debian:bookworm}"
KEEPASS_ATTACHMENT=keepassrx-signing.asc

load_signing_key() {
    local root="$1" pw
    local -a kp=(-q)
    [ -f "$root/.env" ] && . "$root/.env"
    if [ -n "${KEEPASSRX_SIGNING_KDBX:-}" ]; then
        command -v keepassxc-cli >/dev/null || { echo "keepassxc-cli not found" >&2; return 1; }
        [ -n "${KEEPASSRX_SIGNING_ENTRY:-}" ] || { echo "KEEPASSRX_SIGNING_ENTRY is not set" >&2; return 1; }
        [ -n "${KEEPASSRX_SIGNING_KEYFILE:-}" ] && kp+=(-k "$KEEPASSRX_SIGNING_KEYFILE")
        IFS= read -rsp "Password for $KEEPASSRX_SIGNING_KDBX: " pw
        echo >&2
        RPM_SIGNING_KEY="$(printf '%s\n' "$pw" | keepassxc-cli attachment-export "${kp[@]}" --stdout \
            "$KEEPASSRX_SIGNING_KDBX" "$KEEPASSRX_SIGNING_ENTRY" "$KEEPASS_ATTACHMENT")" ||
            { echo "could not read $KEEPASS_ATTACHMENT from $KEEPASSRX_SIGNING_KDBX" >&2; return 1; }
        RPM_SIGNING_PASSPHRASE="$(printf '%s\n' "$pw" | keepassxc-cli show "${kp[@]}" -s -a Password \
            "$KEEPASSRX_SIGNING_KDBX" "$KEEPASSRX_SIGNING_ENTRY")" ||
            { echo "could not read the passphrase from $KEEPASSRX_SIGNING_KDBX" >&2; return 1; }
        pw=
    fi
    if [ -z "${RPM_SIGNING_KEY:-}" ] || [ -z "${RPM_SIGNING_PASSPHRASE:-}" ]; then
        echo "no signing key: set KEEPASSRX_SIGNING_KDBX in .env, or RPM_SIGNING_KEY and RPM_SIGNING_PASSPHRASE" >&2
        return 1
    fi
}

sign_in_container() {
    if ! command -v rpmsign >/dev/null || ! command -v gpg >/dev/null; then
        apt-get update -qq
        DEBIAN_FRONTEND=noninteractive apt-get install -y -qq rpm gnupg >/dev/null
    fi
    GNUPGHOME="$(mktemp -d)"
    export GNUPGHOME
    trap 'gpgconf --kill all 2>/dev/null; rm -rf "$GNUPGHOME"' EXIT
    printf '%s\n' "$RPM_SIGNING_KEY" | gpg --batch --quiet --import
    local id
    id="$(gpg --list-secret-keys --with-colons | awk -F: '/^sec/ { print tolower($5); exit }')"
    if [ -n "${RPM_SIGNING_KEY_ID:-}" ] && [ "${RPM_SIGNING_KEY_ID,,}" != "$id" ]; then
        echo "signing key $id is not the expected key ${RPM_SIGNING_KEY_ID,,}" >&2
        return 1
    fi
    (umask 077 && printf '%s' "$RPM_SIGNING_PASSPHRASE" >"$GNUPGHOME/passphrase")
    local f sig
    local -a owners=()
    for f in "$@"; do owners+=("$(stat -c %u:%g "$f")"); done
    rpmsign --addsign --rpmv3 \
        --define "__gpg $(command -v gpg)" \
        --define "_gpg_name $id" \
        --define "_gpg_sign_cmd_extra_args --batch --pinentry-mode loopback --passphrase-file $GNUPGHOME/passphrase" \
        "$@"
    for f in "$@"; do chown "${owners[0]}" "$f"; owners=("${owners[@]:1}"); done
    gpg --batch --armor --export "$id" >"$GNUPGHOME/public.asc"
    rpm --import "$GNUPGHOME/public.asc"
    for f in "$@"; do
        rpm -K "$f"
        sig="$(rpm -qp --qf '%{SIGPGP:pgpsig}' "$f")"
        case "$sig" in
            *"Key ID $id") echo ">> $(basename "$f"): $sig" ;;
            *) echo "$f is not signed by $id: $sig" >&2; return 1 ;;
        esac
    done
}

sign_rpms() {
    local dir f
    local -a names=()
    dir="$(cd "$(dirname "$1")" && pwd)"
    for f in "$@"; do
        [ "$(cd "$(dirname "$f")" && pwd)" = "$dir" ] || { echo "RPMs must share one folder: $f" >&2; return 1; }
        names+=("/rpms/$(basename "$f")")
    done
    local docker="${DOCKER:-$(command -v docker || command -v podman || true)}"
    [ -n "$docker" ] || { echo "no container runtime (install docker/podman or set \$DOCKER)" >&2; return 1; }
    echo ">> Signing ${names[*]#/rpms/}"
    RPM_SIGNING_KEY="$RPM_SIGNING_KEY" RPM_SIGNING_PASSPHRASE="$RPM_SIGNING_PASSPHRASE" \
        RPM_SIGNING_KEY_ID="${RPM_SIGNING_KEY_ID:-}" \
        "$docker" run --rm -e RPM_SIGNING_KEY -e RPM_SIGNING_PASSPHRASE -e RPM_SIGNING_KEY_ID \
        --tmpfs /tmp -v "$dir":/rpms -v "$(realpath "${BASH_SOURCE[0]}")":/sign.sh:ro \
        "$SIGN_IMAGE" bash /sign.sh --in-container "${names[@]}"
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    if [ "${1:-}" = --in-container ]; then
        shift
        sign_in_container "$@"
        exit
    fi
    root="$(cd "$(dirname "$0")/.." && pwd)"
    shopt -s nullglob
    rpms=("$@")
    [ ${#rpms[@]} -gt 0 ] || rpms=("$root"/RPMS/*.rpm)
    [ ${#rpms[@]} -gt 0 ] || { echo "no RPMs to sign" >&2; exit 1; }
    load_signing_key "$root"
    sign_rpms "${rpms[@]}"
fi
