#!/usr/bin/env bash
# Builds blogawrite and lays it out under a prefix: the binary, the desktop entry
# that maps .md files onto it, and the icon that entry names. Both ends of it want
# the same three files in the same three places — ~/.local for a local install,
# the AppDir's usr for the AppImage — so the prefix is an argument.
set -euo pipefail

prefix=${1:?usage: install.sh <prefix>, e.g. ~/.local}
cd "$(dirname "$0")/.."

# The three package managers this has been built on. Anywhere else the checks below
# still name what is missing; finding the package for it is then the reader's job.
install_with=
if command -v apt-get > /dev/null; then
    distro=apt
    install_with="sudo apt install"
elif command -v dnf > /dev/null; then
    distro=dnf
    install_with="sudo dnf install"
elif command -v pacman > /dev/null; then
    distro=pacman
    install_with="sudo pacman -S"
else
    distro=unknown
fi

missing=()
packages=()

# want <what is missing> <apt> <dnf> <pacman> — unquoted on purpose, so that a
# requirement that takes several packages can name them all.
want() {
    missing+=("$1")
    case $distro in
        apt) packages+=($2) ;;
        dnf) packages+=($3) ;;
        pacman) packages+=($4) ;;
    esac
}

# rust-toolchain.toml pins 1.95, so under rustup the shim has already fetched the
# right toolchain by the time cargo answers here — this is for a cargo from anywhere
# else. Distro Rust is routinely older than harper-core will compile on, so point at
# rustup rather than at a package.
rust_needed=1.95
if ! command -v cargo > /dev/null; then
    missing+=("Rust $rust_needed or newer — https://rustup.rs")
else
    rust_found=$(cargo --version | cut -d' ' -f2)
    # The older of the two sorts first; if that is not the one we need, cargo is older.
    if [ "$(printf '%s\n%s\n' "$rust_needed" "$rust_found" | sort -V | head -1)" != "$rust_needed" ]; then
        missing+=("Rust $rust_needed or newer, found $rust_found — https://rustup.rs")
    fi
fi

# cc builds the C++ half with $CXX, falling back to c++.
command -v "${CXX:-c++}" > /dev/null || want "a C++ compiler" build-essential gcc-c++ base-devel

# qt-build-utils finds Qt by running qmake, from $QMAKE or off the PATH, so a qmake
# that answers is also the evidence that the build will find Qt at all.
qmake=${QMAKE:-$(command -v qmake6 || command -v qmake || true)}
if [ ! -x "$qmake" ]; then
    want "Qt 6.2+ with QtQuick" \
        "qt6-base-dev qt6-declarative-dev" \
        "qt6-qtbase-devel qt6-qtdeclarative-devel" \
        "qt6-base qt6-declarative"
else
    qt_needed=6.2
    qt_found=$("$qmake" -query QT_VERSION)
    if [ "$(printf '%s\n%s\n' "$qt_needed" "$qt_found" | sort -V | head -1)" != "$qt_needed" ]; then
        missing+=("Qt $qt_needed or newer, found $qt_found")
    fi
    # Qt 6 without QtQuick is qt6-base on its own, which is a package short rather
    # than a version behind: the headers this compiles against are in the other one.
    [ -d "$("$qmake" -query QT_INSTALL_HEADERS)/QtQuick" ] ||
        want "the QtQuick headers" qt6-declarative-dev qt6-qtdeclarative-devel qt6-declarative

    # And the QML modules QtQuick loads at runtime whether or not anything imports
    # them by name — split into their own packages on Debian, so a build that
    # succeeds can still come up to an empty window.
    qml=$("$qmake" -query QT_INSTALL_QML)
    if [ ! -f "$qml/QtQuick/qmldir" ] || [ ! -f "$qml/QtQml/WorkerScript/qmldir" ]; then
        want "the QtQuick QML modules" \
            "qml6-module-qtquick qml6-module-qtqml-workerscript qml6-module-qtquick-window qml6-module-qtquick-shapes" \
            qt6-qtdeclarative qt6-declarative
    fi

    # Not fatal: without it Qt falls back to xcb, which every Wayland session can
    # still run this under. Worth saying, since sway is half of who this is for.
    if ! compgen -G "$("$qmake" -query QT_INSTALL_PLUGINS)/platforms/libqwayland*.so" > /dev/null; then
        echo "note: no Qt Wayland platform plugin, so this will run under X11 only —" \
             "install qt6-wayland for a native Wayland window" >&2
    fi
fi

if [ ${#missing[@]} -gt 0 ]; then
    echo "blogawrite needs, and this machine has not got:" >&2
    printf '  %s\n' "${missing[@]}" >&2
    if [ ${#packages[@]} -gt 0 ]; then
        printf '\n  %s %s\n' "$install_with" "$(printf '%s\n' "${packages[@]}" | sort -u | paste -sd' ')" >&2
    fi
    exit 1
fi

cargo build --release

install -Dm755 target/release/blogawrite "$prefix/bin/blogawrite"
install -Dm644 blogawrite.desktop "$prefix/share/applications/blogawrite.desktop"
install -Dm644 packaging/blogawrite.svg \
    "$prefix/share/icons/hicolor/scalable/apps/blogawrite.svg"
