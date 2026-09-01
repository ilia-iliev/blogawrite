# blogawrite

A live-preview markdown editor for Linux, in the style of Typora: what you write
stays rendered while you write it. Prose keeps its typography as you type — bold is
bold, links are links — and the syntax around a construct only comes back into view
when the cursor is inside it. One document, one window, explicit saving, keyboard
only.

Rust core (document model, block segmentation, file I/O) exposed to a Qt Quick UI
through [cxx-qt](https://github.com/KDAB/cxx-qt). Markdown is rendered by Qt itself
via `Text.MarkdownText`, so there is no HTML layer in between. The block being
edited is raw markdown throughout — it is styled, never rewritten, so the file is
exactly what you typed.

## Install

Linux only, and you build it: there are no packages yet.

**Dependencies.** Rust 1.85 or newer (the crate is edition 2024), a C++ compiler, and
Qt 6.2 or newer with the QtQuick and QtQuick.Controls modules — the dev packages, since
the build runs Qt's `qmake6` and `qmlcachegen`. Developed against Qt 6.4.2 and GCC.

Debian / Ubuntu:

```sh
sudo apt install build-essential qt6-base-dev qt6-declarative-dev \
    qml6-module-qtquick qml6-module-qtquick-controls
```

Fedora:

```sh
sudo dnf install gcc-c++ qt6-qtbase-devel qt6-qtdeclarative-devel
```

Arch:

```sh
sudo pacman -S base-devel qt6-base qt6-declarative
```

Rust, if you have not got it: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`.

**Build and install.**

```sh
git clone https://github.com/ilia-iliev/blogawrite.git
cd blogawrite
cargo build --release
install -Dm755 target/release/blogawrite ~/.local/bin/blogawrite
```

Make sure `~/.local/bin` is on your `PATH`. To open `.md` files from a file manager,
install the desktop entry too:

```sh
install -Dm644 blogawrite.desktop ~/.local/share/applications/blogawrite.desktop
```

It is marked `NoDisplay` — there is nothing to launch without a file, so it stays out
of the application menu and only shows up under "Open with".

## Run

```sh
blogawrite sample/post.md
```

The file is required: blogawrite has no way to pick one from inside the app, and
refuses to start without it. A path that does not exist yet starts an empty document
that the first save creates, so `blogawrite new-post.md` is how you begin one.

## Using it

Click anywhere and type. The cursor lands where you clicked and the text keeps
looking the way it reads, so editing a paragraph does not turn it into source.

What each kind of block does while the cursor is in it:

| Block | While you edit it |
| --- | --- |
| paragraph, list | stays rendered; `**`, `*`, `` ` ``, `~~` and `[…](…)` only appear around the construct the cursor is in |
| fenced code | stays rendered, fences hidden — they come back when the cursor reaches the first or last line, on the way out |
| heading, quote, table, rule | opens up into raw markdown: the markup is the structure, and worth seeing |
| lone image | keeps the picture on screen, with its `![…](…)` line underneath to edit |

Keys:

| Key | Does |
| --- | --- |
| Ctrl+S | save |
| Ctrl+B / Ctrl+I / Ctrl+U | wrap the selection in `**`, `*` or `~~`; press again to unwrap |
| Ctrl+Shift+L / Ctrl+Shift+I | `[…](…)` around the selection, or an empty pair to type into — `!` prefixed for the image form |
| PageUp / PageDown | a screenful at a time, keeping the block at the far edge selected |
| Up / Down | at the first or last line, move to the neighbouring block |
| Backspace | at position 0, merge into the previous block |
| Enter twice | end the block and start a new one |

There is always a block under the cursor: Up on the first block and Down on the
last one keep it where it is rather than dropping it.

Unsaved changes show as `●` in the title bar. Closing with unsaved changes puts a
prompt line at the foot of the window — `y` saves and closes, `n` discards, `esc`
goes back to the document. No buttons, nothing to click.

Opening a file puts the cursor back in the block you left it in last time (in the
last block, the first time). Positions are kept in
`~/.local/state/blogawrite/cursors` and recorded on save and on close.

Relative image paths resolve against the document's directory, so `![](image.png)`
next to your post works. A paragraph that is nothing but an image is shown as a
scaled image; images inside a text paragraph render inline.

## Design notes

Live rendering is a `QSyntaxHighlighter` (`cpp/markdownhighlighter.h`, the one piece
of hand-written C++) over the raw markdown in the editor's `QTextDocument`: it walks
the inline markup, gives each character the format its construct implies, and shrinks
the markers themselves away unless the cursor is inside the span they mark. It also
carries the line spacing, which `TextArea` has no property for. Because it only ever
sets character formats, the text under it is untouched. A moved cursor restyles only
the line it left and the line it reached — in a code block, its fences too, which
open and close with the cursor anywhere inside. Rehighlighting the whole block
instead cost 8 ms a keystroke in a long list. Line spacing is the one
thing it writes to the document, as a block format — that is also how a hidden code
fence collapses to nothing, since a line's height is not a character's to give.

The document is a `Vec<Block>` in Rust, exposed to QML as a `QAbstractListModel`.
Blocks come from `pulldown-cmark`'s offset iterator at the top level, sliced out of
the source by byte range, so each block keeps its raw text verbatim — setext
headings, list indentation and code fences all survive untouched. Editing a block
re-segments just that block, which may split it into several. Saving joins the
blocks with a blank line.

## Known limitations

- **Blank-line runs collapse.** Three or more blank lines between blocks become one
  on save. Everything inside a block is preserved exactly, and a document with
  single blank lines between blocks round-trips byte for byte.
- **Undo is per block.** The TextArea's own undo stack, reset when you leave a
  block. There is no document-wide undo.
- **Images inside a paragraph do not show while you edit it.** They render as the
  picture when the cursor is elsewhere; under the cursor the paragraph shows the alt
  text, and the `![…](…)` around it when the cursor reaches it. A QTextDocument
  cannot put a picture into text it does not own.
- **Hidden markers are squeezed, not removed.** Qt has no way to hide characters in
  an editable document, so a marker keeps its glyphs — drawn in nothing, at a
  hundredth of the width they would take. That leaves a fraction of a pixel where
  `**` used to be. Shrinking the font instead would be tidier and is not an option:
  a one-pixel font poisons the glyph atlas Qt Quick shares across the window, and
  text in other blocks comes out as slivers.
- **One file per run.** No open, new or save-as: the document is whatever you named
  on the command line. Editing something else means another `blogawrite`.

## Startup

The QML is compiled ahead of time by `qmlcachegen` and embedded in the binary, the
release profile uses thin LTO and one codegen unit, and the lightweight Basic
Controls style is forced before the engine starts. The app imports only QtQuick and
QtQuick.Controls.Basic, and loads no Dialogs module and no platform theme plugin —
both of which an earlier version pulled in.

The scene graph runs on Qt's software renderer. The OpenGL path spends most of a
launch bringing the graphics driver up — ~170 ms of ~240 ms on the development
machine, before a character is drawn — and buys nothing here: the window is text in
a 720-pixel column, which the software renderer draws in about 6 ms a frame no
matter how wide the window is. Setting `QT_QUICK_BACKEND` or `QSG_RHI_BACKEND`
yourself takes the choice back.

Measured on the development machine (warm cache, X11, timed from `exec` to the first
rendered frame): **≈80 ms**, against ≈240 ms through OpenGL. What is left is about
12 ms of dynamic linking, 7 ms of `QGuiApplication`, 38 ms of QML and window setup —
half of it the font database — and 11 ms to draw. Document size barely registers:
600 blocks start as fast as six, since the list only builds the blocks on screen.

## Tests

`cargo test` covers block segmentation: headings, lists, fenced code, blockquotes,
setext headings, rules, tables, blank-line runs, block-kind naming and lone-image
detection, including the segment → join round trip.

## License

MIT. See [LICENSE](LICENSE).
