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
patchelf_version=0.19.1
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
# linuxdeploy would pack the AppImage too, but its plugin exposes no way to set the
# squashfs block size, which is worth fifty milliseconds of every launch — see below.
# So take the AppDir from linuxdeploy and do the packing a step further down.
if [ ! -x "$tools/appimagetool" ]; then
    curl -fsSL -o "$tools/appimagetool" \
        "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$arch.AppImage"
    chmod +x "$tools/appimagetool"
fi
# The Qt plugin carries a patchelf of its own, and it is 0.15 from 2022. Setting an
# RPATH with it wants two more program headers than the library came with, and it
# grows the table where it stands, at offset 64 — straight over the build-id note and
# into .init, which DT_INIT goes on pointing at. Every library it touches then jumps
# into the middle of its own program headers from _dl_init, before main. There is
# only room to get this wrong where the link is tight enough to put .init a header or
# two past the table: Fedora's Qt is, Ubuntu's has had the slack, which is why this
# has never shown up in CI. So hand both linuxdeploy and its plugin a patchelf that
# leaves .init alone. Pinned rather than tracked like the tools above, because the
# failure it fixes is a segfault before main with nothing in the log to say why.
if [ ! -x "$tools/patchelf" ]; then
    curl -fsSL "https://github.com/NixOS/patchelf/releases/download/$patchelf_version/patchelf-$patchelf_version-$arch.tar.gz" \
        | tar -xzO ./bin/patchelf > "$tools/patchelf"
    chmod +x "$tools/patchelf"
fi
export PATCHELF=$repo/$tools/patchelf

# The AppImages of the tools themselves want FUSE, which build machines rarely have.
export APPIMAGE_EXTRACT_AND_RUN=1
# The Qt plugin's strip is from binutils 2.35, which has never heard of .relr.dyn —
# the packed relative relocations every Fedora library is linked with since 39 — and
# refuses the whole file. Nothing here needs it: the distro ships its Qt already
# stripped, and cargo strips our binary on the way out of the release profile.
export NO_STRIP=1
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
# Leave libxkbcommon to the host. It reads the keyboard and Compose tables out of
# /usr/share, which belong to the host too, and a bundled copy older than those
# tables rejects every keysym it has not heard of — five lines of
# `unrecognized keysym "dead_hamza"` on Fedora 44, one per line of its Compose file
# that a 2022 libxkbcommon cannot read. Library and data have to come from the same
# distro. Every system that can show a window at all has this one.
export LINUXDEPLOY_EXCLUDED_LIBRARIES='libxkbcommon.so.*;libxkbcommon-x11.so.*'
# VERSION is what appimagetool stamps into the .desktop as X-AppImage-Version.
export VERSION=$version

PATH=$tools:$PATH "$tools/linuxdeploy" --appdir "$appdir" --plugin qt

# linuxdeploy brings a whole Qt installation's worth of things this editor cannot
# reach, and every megabyte of it is a megabyte of download and, for the parts the
# loader opens anyway, milliseconds of squashfs. Take them back out, from the leaves
# in: first what the app cannot use, then the libraries nothing left points at.

# The QML module directories go entirely. Our binary links Qt6Quick itself, so
# `import QtQuick` is answered out of the library's own type registration and the
# engine never opens qml/QtQuick/qmldir at all — with the whole tree gone, and the
# host's Qt hidden, it still loads Main.qml. What linuxdeploy put there is every
# module the scanner could reach from QtQuick, Controls and Dialogs and Pdf and
# Particles included; nothing here imports one. The smoke test below is what makes
# this safe to say: a QML file that grows an import this bundle cannot answer fails
# the second run.
rm -rf "$appdir/usr/qml"

# Qt opens every plugin in imageformats at once, the first time anything asks what
# formats it can read — so one `![](picture.png)` in a document pulls in the lot. On
# a machine with kimageformats installed that is twenty-six plugins and, behind them,
# four AV1 encoders, OpenEXR and a raw-photo decoder: 90MB, read off the squashfs, to
# be told none of them is what the document points at. Keep what a blog post uses.
for plugin in "$appdir"/usr/plugins/imageformats/*; do
    case ${plugin##*/} in
        libqgif.so | libqico.so | libqjpeg.so | libqsvg.so | libqwebp.so) ;;
        *) rm -f "$plugin" ;;
    esac
done

# Qt's own translations, for an app that installs no QTranslator and never asks.
rm -rf "$appdir/usr/translations"

# Now the libraries. Ask the loader itself what is still reachable — from the binary
# and from every plugin left standing, resolved through the same RPATHs it will use
# at runtime — and drop the rest. This is where deleting a plugin above turns into
# megabytes: libQt6Pdf and Qt6Widgets went out with the PDF image format, the AV1
# and JPEG XL encoders with kimageformats. Nothing here is dlopened behind ldd's
# back: libproxy's backend, the one library that would be, names itself in DT_NEEDED.
reachable=$work/reachable
{
    ldd "$appdir/usr/bin/blogawrite"
    find "$appdir/usr/plugins" -name '*.so' \
        -exec env LD_LIBRARY_PATH="$appdir/usr/lib" ldd {} \;
} 2>/dev/null | awk '/=> \//{print $3}' | xargs -r -n64 realpath -m -- | sort -u > "$reachable"
find "$appdir/usr/lib" -maxdepth 1 -type f | while read -r lib; do
    grep -qxF "$(realpath -m "$lib")" "$reachable" || rm -f "$lib"
done

# The AppImage is a squashfs read through FUSE, so every library the loader touches is
# decompressed on the way in, on every launch. That is the single largest part of the
# startup: about 145ms of the 400 it takes to put a window up. The default 128K block
# is the wrong shape for it — a shared library is read in scattered pieces, and each
# read pays for a whole block. Sixteen 32K blocks cost less than four 128K ones when
# only a fraction of each is wanted: 50ms off every launch, for 2MB more to download.
# Uncompressed is faster still, and nearly three times the download; not worth it.
"$tools/appimagetool" --no-appstream \
    --mksquashfs-opt -b --mksquashfs-opt 32K \
    "$appdir" "$output"

# Anything missing from the bundle only shows up when something goes looking for it,
# which is to say on the machine of whoever downloads this. So go looking here.
smoke=$work/smoke
rm -rf "$smoke"
mkdir -p "$smoke/home"
cp sample/image.png "$smoke/image.png"
printf '# Heading\n\nSome **bold** text, a [link](https://example.com) and `code`.\n\n![a picture](image.png)\n' \
    > "$smoke/post.md"

# Open the document offscreen and insist the window is still standing fifteen seconds
# on. The image is in there so that the run reaches the image plugins too.
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

# Standing there is weaker evidence than it looks: an engine that cannot build the
# root object prints its complaint and leaves the process running with no window,
# which from out here is indistinguishable from success — and is how a bundle missing
# half its QML would pass. So make the QML answer for itself. A directory is the one
# thing Document::open_path refuses, Main.qml turns that false into Qt.exit, and the
# process comes straight back. Only a loaded root object running its JavaScript can
# do that, so a run that instead sits there until the timeout is the engine failing.
set +e
timeout 15 env HOME=$repo/$smoke/home QT_QPA_PLATFORM=offscreen \
    "$output" "$smoke/home" > "$smoke/qml-log" 2>&1
status=$?
set -e
if [ $status -ne 0 ]; then
    echo "smoke test: Main.qml never got as far as refusing a directory (exit $status)" >&2
    cat "$smoke/qml-log" >&2
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

for lib in "$appdir"/usr/lib/libxkbcommon*; do
    if [ -e "$lib" ]; then
        echo "smoke test: $(basename "$lib") got bundled; it has to stay the host's" >&2
        exit 1
    fi
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
