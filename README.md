# blogawrite

A live-preview markdown editor for Linux, in the style of Typora. Bold is bold, links
are links, and the markup around a construct only appears when the cursor is inside
it. The buffer stays raw markdown — it is styled, never rewritten — so the file is
exactly what you typed.

One document, one window, explicit saving, keyboard only. Rust core over a Qt Quick
UI via [cxx-qt](https://github.com/KDAB/cxx-qt), rendered by Qt's own
`Text.MarkdownText`.

## Install

One file, which carries its own Qt. There is nothing else to install, and nothing
left behind afterwards but the file itself.

```sh
curl -LO https://github.com/ilia-iliev/blogawrite/releases/latest/download/blogawrite-x86_64.AppImage
chmod +x blogawrite-x86_64.AppImage
./blogawrite-x86_64.AppImage post.md
```

That URL always serves the newest release — the name carries no version, so it does
not go stale. Which version you ended up with is on the [releases
page](https://github.com/ilia-iliev/blogawrite/releases), and inside the file as
`X-AppImage-Version`. To check the download, `SHA256SUMS` sits beside it:

```sh
curl -LO https://github.com/ilia-iliev/blogawrite/releases/latest/download/SHA256SUMS
sha256sum -c SHA256SUMS
```

To keep it, put it on your path under the plain name, which is what the desktop file's
`Exec=` expects — install that too and markdown files start offering it in "Open with":

```sh
install -Dm755 blogawrite-x86_64.AppImage ~/.local/bin/blogawrite
curl -fsSLO https://raw.githubusercontent.com/ilia-iliev/blogawrite/main/blogawrite.desktop
install -Dm644 blogawrite.desktop ~/.local/share/applications/blogawrite.desktop
```

Built against glibc 2.35, which means Debian 12, Ubuntu 22.04, Fedora 36 and anything
newer. x86_64 only. Under Wayland it draws through XWayland.

## Build from source

Needs Rust 1.85+, a C++ compiler, and Qt 6.2+ dev packages with QtQuick and
QtQuick.Controls.

```sh
# Debian / Ubuntu — the QML modules are split across packages, and QtQuick pulls in
# the last four at runtime whether or not you import them yourself
sudo apt install build-essential qt6-base-dev qt6-declarative-dev \
    qml6-module-qtquick qml6-module-qtquick-controls \
    qml6-module-qtqml-workerscript qml6-module-qtquick-templates \
    qml6-module-qtquick-window qml6-module-qtquick-shapes
# Fedora
sudo dnf install gcc-c++ qt6-qtbase-devel qt6-qtdeclarative-devel
# Arch
sudo pacman -S base-devel qt6-base qt6-declarative
```

```sh
cargo build --release
install -Dm755 target/release/blogawrite ~/.local/bin/blogawrite
install -Dm644 blogawrite.desktop ~/.local/share/applications/blogawrite.desktop
install -Dm644 packaging/blogawrite.svg \
    ~/.local/share/icons/hicolor/scalable/apps/blogawrite.svg
```

`packaging/build-appimage.sh` builds the AppImage itself: it packs the release binary
together with the Qt it needs, then opens a document offscreen to check the bundle is
whole before calling it done. Build it on the oldest distro you mean to support —
glibc is the one thing an AppImage cannot bring along. Tagging `v*` runs the same
script on Ubuntu 22.04 and attaches the result to a GitHub release.

## Run

```sh
blogawrite sample/post.md
```

The path is required — there is no file picker. A path that does not exist yet opens
an empty document that the first save creates.

## Using it

How each block behaves while the cursor is in it:

| Block | While you edit it |
| --- | --- |
| paragraph, list | stays rendered; `**`, `*`, `` ` ``, `~~` and `[…](…)` show only around the construct under the cursor |
| fenced code | stays rendered, fences hidden until the cursor reaches the first or last line |
| heading, quote, table, rule | opens up into raw markdown |
| lone image | keeps the picture, with its `![…](…)` line underneath |

| Key | Does |
| --- | --- |
| Ctrl+S | save |
| Ctrl+B / Ctrl+I / Ctrl+U | wrap the selection in `**`, `*` or `~~`; again to unwrap |
| Ctrl+Shift+L / Ctrl+Shift+I | `[…](…)` around the selection, or an empty pair; `!` prefixed for images |
| PageUp / PageDown | a screenful, selecting the block at the far edge |
| Up / Down | at the first or last line, move to the neighbouring block |
| Backspace | at position 0, merge into the previous block |
| Enter twice | end the block and start a new one |

Closing with unsaved changes prompts at the foot of the window: `y` saves and closes,
`n` discards, `esc` goes back. Cursor positions are remembered per file in
`~/.local/state/blogawrite/cursors`.

## Known limitations

- **Blank-line runs collapse.** Three or more blank lines between blocks become one on
  save. Block contents are preserved exactly.
- **Undo is per block.** The TextArea's own undo stack, reset when you leave a block.
- **Images inside a paragraph do not show while you edit it.** A QTextDocument cannot
  put a picture into text it does not own, so you get the alt text instead.
- **Hidden markers are squeezed, not removed.** Qt cannot hide characters in an
  editable document, so a marker is drawn in nothing at a hundredth of its width,
  leaving a fraction of a pixel.
- **One file per run.** No open, new or save-as.

## License

MIT. See [LICENSE](LICENSE).
