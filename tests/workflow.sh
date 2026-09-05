#!/usr/bin/env bash
# What the checker is for, driven from outside the window: type something wrong, pause,
# watch it get marked, walk what is offered, take one. A typo and a clumsy turn of phrase
# go through the very same steps, which is the point of the exercise — so both are typed,
# and both are asked for the same things.
#
# Needs Xvfb, xdotool, xclip and ImageMagick. No window manager: the editor is the only window
# there is, and the root window is what gets photographed. Ctrl+S is a Qt shortcut and
# wants a window a window manager has made active, so the text is read back off the
# clipboard, which the editor puts there itself.
#
#     cargo build && tests/workflow.sh

set -euo pipefail

readonly root="$(cd "$(dirname "$0")/.." && pwd)"
readonly editor="${1:-$root/target/debug/blogawrite}"
# The two colours worth counting, read from where the palette is written down: the wash
# behind anything the checker objects to, and the foot of the window it says so in.
colour() { grep -oP "$1: QString::from\(\"\\K#[0-9A-Fa-f]{6}" "$root/src/theme.rs"; }
readonly wash="$(colour lint)"
readonly foot="$(colour prompt_background)"
# Long enough for the checker to have had its say: the editor waits 600ms of quiet before
# it says anything, and then has to draw it.
readonly settle=1.5

readonly work="$(mktemp -d)"
readonly document="$work/document.md"
readonly display=":97"
export DISPLAY="$display"

failures=0

cleanup() {
    kill "${app:-}" 2>/dev/null || true
    kill "${xvfb:-}" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT

# How many pixels of the window are painted in `$1`.
painted() {
    import -window root "$work/shot.png"
    convert "$work/shot.png" -format %c histogram:info:- \
        | awk -F: -v colour="$1" '$0 ~ colour { gsub(/ /, "", $1); print $1; found = 1; exit }
                                  END { if (!found) print 0 }'
}

marked() { painted "$wash"; }
offering() { painted "$foot"; }

type_in() { xdotool type --delay 12 "$1" >/dev/null; }
press() { xdotool key --delay 60 "$@" >/dev/null; }

# What the editor holds, by way of its own copy: select the document, copy it, put the
# cursor back where the text ends.
written() {
    press ctrl+a
    press ctrl+c
    sleep 0.4
    press Right
    xclip -selection clipboard -o 2>/dev/null
}

# Run one check, and let it say what it found instead when it does not hold.
check() {
    local what="$1"
    local reason
    shift
    if reason="$("$@")"; then
        echo "  ok    $what"
    else
        echo "  FAIL  $what — $reason"
        failures=$((failures + 1))
    fi
}

none() {
    [ "$1" = 0 ] && return 0
    echo "$1 pixels of it are there"
    return 1
}

some() {
    [ "$1" -gt 20 ] && return 0
    echo "there is none of it on screen"
    return 1
}

reads() {
    [ "$1" = "$2" ] && return 0
    echo "it reads \"$1\""
    return 1
}

printf '' > "$document"
mkdir -p "$work/home"
Xvfb "$display" -screen 0 1200x900x24 >/dev/null 2>&1 &
xvfb=$!
sleep 1

HOME="$work/home" XDG_CONFIG_HOME="$work/config" \
    DICPATH="$work/no-system-dictionaries" "$editor" "$document" >/dev/null 2>&1 &
app=$!
# The checker's rules and dictionaries take the better part of a second to load, and
# nothing is marked before they are there.
sleep 3

echo "a typo"
type_in "I recieve mail"
check "nothing is marked while the typing is still going on" none "$(marked)"
sleep "$settle"
check "the typo is marked once the typing stops" some "$(marked)"
check "and nothing is offered until the cursor stands in it" none "$(offering)"

# Back into the word: the checker offers what to do about the words the cursor is in.
# Straight away — the pause is for typing, and moving the cursor is not typing.
press Left Left Left Left Left
sleep 0.4
check "standing in it offers what was meant instead" some "$(offering)"

press ctrl+Return
sleep "$settle"
check "accepting puts the dictionary's word in" reads "$(written)" "I receive mail"
check "and there is nothing left to mark" none "$(marked)"

echo
echo "a turn of phrase"
type_in ". This is very unique"
sleep "$settle"
check "the phrase is marked the same way" some "$(marked)"
check "and offered the same way" some "$(offering)"

# Three forward and two back is one forward, however many the checker offered, and its
# second thought about `very unique` is `very rare`.
press ctrl+Down ctrl+Down ctrl+Down ctrl+Up ctrl+Up
press ctrl+Return
sleep "$settle"
check "the suggestion walked to is the one accepted" \
    reads "$(written)" "I receive mail. This is very rare"
check "and there is nothing left to mark" none "$(marked)"

echo
echo "what was not typed wrong"
type_in ". Call \`recieve_this\` at exampel.com"
sleep "$settle"
check "code and addresses are left alone" none "$(marked)"

type_in ". See [the exampel site](http://a.test/pge)"
sleep "$settle"
check "and so are links, text and all" none "$(marked)"

echo
echo "a word of the writer's own"
type_in ". Blogawrite"
sleep "$settle"
check "a name the dictionary has never heard of is marked" some "$(marked)"

press ctrl+shift+Return
sleep "$settle"
check "keeping it takes the mark off" none "$(marked)"
check "and writes it down for next time" \
    reads "$(cat "$work/config/blogawrite/dictionary" 2>/dev/null)" "Blogawrite"

echo
echo "a table"
press Return Return
type_in "| Naem | Vlaue |"
press Return
type_in "| --- | --- |"
press Return
type_in "| tpyo | anohter |"
sleep "$settle"
check "a table is a table, not prose" none "$(marked)"

echo
if [ "$failures" -gt 0 ]; then
    echo "$failures failed"
    exit 1
fi
echo "all passed"
