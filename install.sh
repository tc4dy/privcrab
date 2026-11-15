set -euo pipefail

REPO="https://github.com/tc4dy/privcrab"
WORKDIR=""
PM=""

cleanup() {
    if [ -n "$WORKDIR" ]; then
        rm -rf "$WORKDIR"
    fi
}
trap cleanup EXIT
trap 'exit 130' INT TERM

fail() {
    echo "[!] $1" >&2
    exit 1
}

as_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    else
        sudo "$@"
    fi
}

pkg_for() {
    case "$PM:$1" in
        pacman:cargo) echo rust ;;
        *:cargo) echo cargo ;;
        *:cc) echo gcc ;;
        *:git) echo git ;;
        *:curl) echo curl ;;
        apt-get:pcap) echo libpcap-dev ;;
        pacman:pcap) echo libpcap ;;
        *) echo libpcap-devel ;;
    esac
}

install_pkg() {
    case "$PM" in
        apt-get) as_root apt-get update -qq && as_root apt-get install -y "$1" ;;
        dnf) as_root dnf install -y "$1" ;;
        pacman) as_root pacman -S --noconfirm --needed "$1" ;;
        zypper) as_root zypper --non-interactive install "$1" ;;
        *) return 1 ;;
    esac
}

get_pkg() {
    local pkg
    if [ -z "$PM" ]; then
        echo "[>] no supported package manager found (apt, dnf, pacman, zypper)" >&2
        echo "[>] install '$1' manually and run this script again" >&2
        exit 1
    fi
    pkg="$(pkg_for "$1")"
    install_pkg "$pkg" || fail "could not install $pkg"
}

need_cmd() {
    if command -v "$1" >/dev/null 2>&1; then
        return 0
    fi
    echo "[!] missing: $1"
    get_pkg "$2"
    command -v "$1" >/dev/null 2>&1 || fail "$1 not found after install"
}

pcap_ok() {
    if [ ! -e /usr/include/pcap.h ] && [ ! -e /usr/include/pcap/pcap.h ]; then
        return 1
    fi
    if command -v ldconfig >/dev/null 2>&1 && ldconfig -p 2>/dev/null | grep -q 'libpcap\.so'; then
        return 0
    fi
    for dir in /usr/lib /usr/lib64 /usr/local/lib /usr/local/lib64; do
        if [ -e "$dir/libpcap.so" ]; then
            return 0
        fi
    done
    return 1
}

echo "[*] privcrab installer"

if [ "${BASH_VERSINFO[0]}" -lt 4 ]; then
    fail "bash 4 or newer is required"
fi

if [ "$(uname -s)" != "Linux" ]; then
    fail "unsupported OS"
fi

case "$(uname -m)" in
    x86_64|aarch64) ;;
    *) fail "unsupported architecture: $(uname -m)" ;;
esac

if [ "$(id -u)" -ne 0 ] && ! command -v sudo >/dev/null 2>&1; then
    fail "run as root or install sudo"
fi

for candidate in apt-get dnf pacman zypper; do
    if command -v "$candidate" >/dev/null 2>&1; then
        PM="$candidate"
        break
    fi
done

if [ -n "${SUDO_USER:-}" ] && [ -f "/home/$SUDO_USER/.cargo/env" ]; then
    . "/home/$SUDO_USER/.cargo/env"
elif [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "[*] installing rustup (cargo not found)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
fi

rust_ver="$(rustc --version | awk '{print $2}')" || fail "could not read rustc version"
IFS=. read -r rust_major rust_minor _ <<< "$rust_ver"
if [ "$rust_major" -lt 1 ] || { [ "$rust_major" -eq 1 ] && [ "$rust_minor" -lt 74 ]; }; then
    echo "[!] rust $rust_ver is too old (need 1.74+)"
    echo "[*] installing rustup to get a newer toolchain"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
    rust_ver="$(rustc --version | awk '{print $2}')"
    IFS=. read -r rust_major rust_minor _ <<< "$rust_ver"
    if [ "$rust_major" -lt 1 ] || { [ "$rust_major" -eq 1 ] && [ "$rust_minor" -lt 74 ]; }; then
        fail "rust 1.74 or newer is required, found $rust_ver"
    fi
fi

if ! command -v curl >/dev/null 2>&1 && ! command -v wget >/dev/null 2>&1; then
    echo "[!] missing: curl or wget"
    get_pkg curl
fi

need_cmd git git
need_cmd cc cc
need_cmd cargo cargo

if ! pcap_ok; then
    echo "[!] missing: libpcap development files"
    get_pkg pcap
    if ! pcap_ok; then
        fail "libpcap development files not found after install"
    fi
fi

if command -v curl >/dev/null 2>&1; then
    curl -fsSI --max-time 20 "$REPO" >/dev/null || fail "cannot reach github.com"
else
    wget -q --spider --timeout=20 "$REPO" || fail "cannot reach github.com"
fi

WORKDIR="/tmp/privcrab-install-$$"
mkdir -m 700 "$WORKDIR" || fail "could not create $WORKDIR"

echo "[*] cloning $REPO"
git clone --depth 1 "$REPO" "$WORKDIR/src" || fail "clone failed"

echo "[*] building"
( cd "$WORKDIR/src" && cargo build --release --locked ) || fail "build failed - see output above"

BIN="$WORKDIR/src/target/release/privcrab"
[ -x "$BIN" ] || fail "build finished but $BIN is missing"

echo "[*] installing to /usr/local/bin"
as_root mkdir -p /usr/local/bin || fail "could not create /usr/local/bin"
as_root cp "$BIN" /usr/local/bin/privcrab.new || fail "copy failed"
as_root chmod 755 /usr/local/bin/privcrab.new || fail "chmod failed"
as_root mv -f /usr/local/bin/privcrab.new /usr/local/bin/privcrab || fail "could not replace /usr/local/bin/privcrab"

if command -v setcap >/dev/null 2>&1; then
    if as_root setcap cap_net_admin,cap_net_raw+ep /usr/local/bin/privcrab; then
        echo "[+] cap_net_admin,cap_net_raw set — MAC and probe modules run without sudo"
        echo "[>] dns and hostname modules still need root"
    else
        echo "[!] setcap failed - run privcrab with sudo"
    fi
else
    echo "[!] setcap not found - run privcrab with sudo"
fi

echo "[+] privcrab installed - run: privcrab --help"