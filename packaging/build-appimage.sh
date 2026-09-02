#!/usr/bin/env bash
# Builds dist/blogawrite-<arch>.AppImage: the release binary with the Qt
# libraries and QML modules it needs packed in beside it, so that downloading it is
# the whole install. Run it on the oldest distro you want the AppImage to run on —
# glibc is the one thing it cannot bring along.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"

version=$(sed -n '0,/^version = "\(.*\)"/s//\1/p' Cargo.toml)
arch=$(uname -m)
work=target/appimage
appdir=$work/AppDir
tools=$work/tools
# No version in the name, so that the release page's /latest/download/ URL for it
# is a link that keeps working. The version rides along inside, as X-AppImage-Version.
output=dist/blogawrite-$arch.AppImage

cargo build --release

rm -rf "$appdir"
install -Dm755 target/release/blogawrite "$appdir/usr/bin/blogawrite"
install -Dm644 blogawrite.desktop "$appdir/usr/share/applications/blogawrite.desktop"
install -Dm644 packaging/blogawrite.svg "$appdir/usr/share/icons/hicolor/scalable/apps/blogawrite.svg"

mkdir -p "$tools" dist
for tool in linuxdeploy linuxdeploy-plugin-qt; do
    if [ ! -x "$tools/$tool" ]; then
        curl -fsSL -o "$tools/$tool" \
            "https://github.com/linuxdeploy/$tool/releases/download/continuous/$tool-$arch.AppImage"
        chmod +x "$tools/$tool"
    fi
done

# The AppImages of the tools themselves want FUSE, which build machines rarely have.
export APPIMAGE_EXTRACT_AND_RUN=1
# The Qt plugin finds Qt through qmake, and reads the QML imports to know which QML
# modules to bring. Ours are compiled into the binary, so point it at the sources.
export QMAKE=${QMAKE:-$(command -v qmake6)}
if [ ! -x "${QMAKE:-}" ]; then
    echo "qmake6 not on PATH — set QMAKE to it, or install the Qt 6 dev packages" >&2
    exit 1
fi
export QML_SOURCES_PATHS=$repo/qml

# One AppImage for both display servers: xcb comes by default, and Qt picks between
# the two at startup — wayland when it is a Wayland session, xcb otherwise. Qt has
# renamed these plugins along the way (6.2 splits them into libqwayland-generic.so
# and libqwayland-egl.so, later versions merge them into libqwayland.so), so take
# whichever ones this Qt has rather than naming them. Offscreen is what the smoke
# test below draws into.
platforms=$("$QMAKE" -query QT_INSTALL_PLUGINS)/platforms
# `|| true`: with no match ls exits non-zero, and set -e would end the run here
# rather than at the message below, which is the one worth reading.
wayland=$(cd "$platforms" && ls libqwayland*.so 2>/dev/null | paste -sd';') || true
if [ -z "$wayland" ]; then
    echo "no Qt Wayland platform plugin in $platforms — install qt6-wayland" >&2
    exit 1
fi
export EXTRA_PLATFORM_PLUGINS="libqoffscreen.so;$wayland"
# Deploying a libqwayland* platform plugin brings the window decorations and the
# shell integration along with it. This adds the third directory, the client-side
# graphics integration that puts Qt Quick on the GPU: the module is named for the
# compositor, but what it deploys is the client's half.
export EXTRA_QT_MODULES=waylandcompositor
export LDAI_OUTPUT=$output
export VERSION=$version

PATH=$tools:$PATH "$tools/linuxdeploy" --appdir "$appdir" --plugin qt --output appimage

# A QML module missing from the bundle only shows up when the engine goes looking for
# it, which is to say on the machine of whoever downloads this. So go looking here:
# open a document offscreen and insist the window is still standing fifteen seconds on.
smoke=$work/smoke
rm -rf "$smoke"
mkdir -p "$smoke/home"
printf '# Heading\n\nSome **bold** text, a [link](https://example.com) and `code`.\n' > "$smoke/post.md"
set +e
timeout 15 env HOME=$repo/$smoke/home QT_QPA_PLATFORM=offscreen \
    "$output" "$smoke/post.md" > "$smoke/log" 2>&1
status=$?
set -e
if [ $status -ne 124 ]; then
    echo "smoke test: the AppImage exited with $status instead of staying up" >&2
    cat "$smoke/log" >&2
    exit 1
fi
if grep -Eiq 'not installed|failed to load|no such file|could not find' "$smoke/log"; then
    echo "smoke test: Qt could not find something it needs" >&2
    cat "$smoke/log" >&2
    exit 1
fi

# The build machine has no compositor to open a Wayland window on, and the run above
# went to offscreen, so neither would notice the Wayland half of the bundle going
# missing — which is how it came to be missing for the first two releases. Check it
# is there instead, and that nothing it pulls in was left behind.
wanted=(wayland-shell-integration/libxdg-shell.so
        wayland-graphics-integration-client/libqt-plugin-wayland-egl.so)
for platform in ${wayland//;/ }; do
    wanted+=("platforms/$platform")
done

for plugin in "${wanted[@]}"; do
    path=$appdir/usr/plugins/$plugin
    if [ ! -f "$path" ]; then
        echo "smoke test: $plugin is not in the bundle" >&2
        exit 1
    fi
    if LD_LIBRARY_PATH=$appdir/usr/lib ldd "$path" | grep -q 'not found'; then
        echo "smoke test: $plugin is missing a library it needs" >&2
        LD_LIBRARY_PATH=$appdir/usr/lib ldd "$path" | grep 'not found' >&2
        exit 1
    fi
done

echo "built $output"
