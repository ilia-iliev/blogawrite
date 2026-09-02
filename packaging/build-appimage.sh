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
export QML_SOURCES_PATHS=$repo/qml
# xcb comes by default — under Wayland the app runs through XWayland. Offscreen is
# what the smoke test below draws into.
export EXTRA_PLATFORM_PLUGINS=libqoffscreen.so
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

echo "built $output"
