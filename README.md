# blogawrite

A minimal Markdown editor for tiling manager like i3/sway. You type Markdown and it renders.

## Install

```sh
curl -LO https://github.com/ilia-iliev/blogawrite/releases/latest/download/blogawrite-x86_64.AppImage
chmod +x blogawrite-x86_64.AppImage
./blogawrite-x86_64.AppImage post.md
```

That URL always serves the newest release. `SHA256SUMS` sits beside it on the [releases page](https://github.com/ilia-iliev/blogawrite/releases)

## Checking

American-English spelling and style checking are built into blogawrite. They work without a system dictionary or another package. Words you keep are stored in your personal blogawrite dictionary.

## Build from source

Rust 1.95+, a C++ compiler, and Qt 6.2+ with QtQuick

```sh
# Debian / Ubuntu — QtQuick pulls in the last three at runtime whether you import
# them yourself or not
sudo apt install build-essential qt6-base-dev qt6-declarative-dev qt6-wayland \
    qml6-module-qtquick qml6-module-qtqml-workerscript \
    qml6-module-qtquick-window qml6-module-qtquick-shapes
# Fedora
sudo dnf install gcc-c++ qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtwayland
# Arch
sudo pacman -S base-devel qt6-base qt6-declarative qt6-wayland
```

```sh
cargo build --release
install -Dm755 target/release/blogawrite ~/.local/bin/blogawrite
install -Dm644 blogawrite.desktop ~/.local/share/applications/blogawrite.desktop
install -Dm644 packaging/blogawrite.svg \
    ~/.local/share/icons/hicolor/scalable/apps/blogawrite.svg
```

## License

MIT pen
