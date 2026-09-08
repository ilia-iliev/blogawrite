# blogawrite

A minimal Markdown editor for tiling managers like i3/sway. You type Markdown and it renders. Keyboard-first, no clicking required.
The block with the live cursor is raw Markdown where rendering doesn't make sense. The other blocks are rendered.
Blogawrite explicitly requires a filename and opens one file at a time. The tiling manager is assumed to handle tabs/multiple instances.
Blogawrite supports the main Markdown primitives such as inline and code blocks, links, headings, and images:


![screenshot of blogawrite](packaging/screenshot.png)

*The block with the live cursor is raw Markdown where rendering doesn't make sense*

## Install

```sh
curl -LO https://github.com/ilia-iliev/blogawrite/releases/latest/download/blogawrite-x86_64.AppImage
chmod +x blogawrite-x86_64.AppImage
./blogawrite-x86_64.AppImage post.md
```

## Grammar Checking

American-English spelling and style checking are built into. Extendable personal dictionary included.

## Build from source

```sh
packaging/install.sh ~/.local
```

It checks for Rust, a C++ compiler and Qt 6 with QtQuick first

## License

MIT
