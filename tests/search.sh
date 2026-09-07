#!/usr/bin/env bash
# What the search is for, driven from outside the window: open it, type a word, watch the
# cursor land on it, walk the occurrences round the document and back, and close it again.
# Which occurrence the cursor is standing on is read off the clipboard — the selection is
# stretched to the end of its line first, so that three occurrences of the same word are
# told apart by what follows them.
#
# Runs in a headless sway of its own, which is the compositor this editor is written for.
# Needs sway, wtype, grim and wl-clipboard. No other window: sway tiles the editor over
# the whole output, and that output is what gets photographed.
#
#     cargo build && tests/search.sh

set -euo pipefail

readonly root="$(cd "$(dirname "$0")/.." && pwd)"
readonly editor="${1:-$root/target/debug/blogawrite}"
# The two colours worth counting, read from where the palette is written down: the foot of
# the window, which the search bar fills while it is open, and the ground the notice about
# a word that turns up only once is put on. Nothing in the document below is drawn in
# either — it is plain prose, with no markup for the accent to land on.
colour() { grep -oP "$1: QString::from\(\"\\K#[0-9A-Fa-f]{6}" "$root/src/theme.rs"; }
readonly foot="$(colour prompt_background)"
readonly accent="$(colour accent)"

readonly work="$(mktemp -d)"
readonly document="$work/document.md"

failures=0

cleanup() {
    kill "${app:-}" 2>/dev/null || true
    kill "${compositor:-}" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT

# ImageMagick 7 renamed `convert` to `magick` and warns on every use of the old name.
readonly imagemagick="$(command -v magick || command -v convert)"

# How many pixels of the output are painted in `$1`.
painted() {
    grim "$work/shot.png"
    "$imagemagick" "$work/shot.png" -format %c histogram:info:- \
        | awk -F: -v colour="$1" '$0 ~ colour { gsub(/ /, "", $1); print $1; found = 1; exit }
                                  END { if (!found) print 0 }'
}

open_at_foot() { painted "$foot"; }
noticed() { painted "$accent"; }

# wtype makes a virtual keyboard of its own for every run, and the first keystroke of a
# run is lost while the compositor is still taking in its keymap. The pause before each
# one is what the keymap gets through in.
readonly settle_keymap=150

type_in() { wtype -s "$settle_keymap" -d 12 -- "$1"; }

# One keystroke, named the way xkb names it, with `ctrl` and `shift` for the modifiers:
#
#     press ctrl Down     press shift End     press Escape
press() {
    local held=()
    while [ "$#" -gt 1 ]; do
        held+=(-M "$1")
        shift
    done
    local release=()
    local modifier
    for modifier in "${held[@]}"; do
        [ "$modifier" = -M ] || release=(-m "$modifier" "${release[@]}")
    done
    wtype -s "$settle_keymap" "${held[@]}" -k "$1" "${release[@]}"
    sleep 0.15
}

# What the cursor is standing on, and the rest of the line it is standing in: the search
# leaves the word it found selected, pinned at its start, so shift+End stretches that
# selection to the end of the line without moving where it began.
under_cursor() {
    press shift End
    press ctrl c
    sleep 0.4
    wl-paste --no-newline 2>/dev/null
}

# Open the search on `$1`, as a writer would: the bar comes up, then the word is typed.
look_for() {
    press ctrl f
    sleep 0.3
    type_in "$1"
    sleep 0.5
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

# Three occurrences of one word, one to a block, each with something different after it;
# and a word that turns up once. Plain prose throughout, so the checker has nothing to say
# and the foot of the window stays empty until the search is opened.
cat > "$document" <<'DOCUMENT'
One marker here.

Two marker there.

A solitary marker word.
DOCUMENT

mkdir -p "$work/home" "$work/run"
chmod 700 "$work/run"
export XDG_RUNTIME_DIR="$work/run"
# A compositor of our own, drawing into memory rather than onto a screen or a card.
export WLR_BACKENDS=headless
export WLR_RENDERER=pixman
export WLR_LIBINPUT_NO_DEVICES=1
unset WAYLAND_DISPLAY DISPLAY

cat > "$work/sway.conf" <<'CONFIG'
output HEADLESS-1 resolution 1200x900
default_border none
default_floating_border none
CONFIG

sway -c "$work/sway.conf" >"$work/sway.log" 2>&1 &
compositor=$!
# Its socket is the only one under our own runtime directory, so it says when it is up.
socket=""
for _ in $(seq 40); do
    for candidate in "$XDG_RUNTIME_DIR"/wayland-*; do
        case "$candidate" in
        *.lock | *"*") ;;
        *) socket="$candidate" ;;
        esac
    done
    if [ -n "$socket" ]; then
        break
    fi
    sleep 0.25
done
if [ -z "$socket" ]; then
    echo "sway did not come up:"
    cat "$work/sway.log"
    exit 1
fi
export WAYLAND_DISPLAY="$(basename "$socket")"

HOME="$work/home" XDG_CONFIG_HOME="$work/config" XDG_STATE_HOME="$work/state" \
    DICPATH="$work/no-system-dictionaries" "$editor" "$document" >/dev/null 2>&1 &
app=$!
# The checker's rules and dictionaries take the better part of a second to load, and the
# foot of the window is not settled until they are there.
sleep 3

echo "opening and closing"
check "the foot of the window is empty to begin with" none "$(open_at_foot)"
press ctrl f
sleep 0.4
check "ctrl+f brings the search bar up" some "$(open_at_foot)"
press ctrl f
sleep 0.4
check "and ctrl+f again takes it down" none "$(open_at_foot)"
press ctrl f
sleep 0.4
press Escape
sleep 0.4
check "so does escape" none "$(open_at_foot)"
press ctrl f
sleep 0.4
press Return
sleep 0.4
check "and so does enter, the word being typed" none "$(open_at_foot)"

echo
echo "the word typed"
look_for "marker"
press Escape
sleep 0.3
check "the first occurrence is the one under the cursor" \
    reads "$(under_cursor)" "marker here."

echo
echo "walking the occurrences"
look_for "marker"
press ctrl Down
press Escape
sleep 0.3
check "ctrl+down goes to the next one" reads "$(under_cursor)" "marker there."

look_for "marker"
press ctrl Down
press ctrl Down
press Escape
sleep 0.3
check "and on to the one after that" reads "$(under_cursor)" "marker word."

look_for "marker"
press ctrl Down
press ctrl Down
press ctrl Down
press Escape
sleep 0.3
check "walking off the end comes back round to the first" \
    reads "$(under_cursor)" "marker here."

look_for "marker"
press ctrl Up
press Escape
sleep 0.3
check "ctrl+up from the first goes the other way, round to the last" \
    reads "$(under_cursor)" "marker word."

look_for "marker"
press ctrl Down
press ctrl Up
press Escape
sleep 0.3
check "one each way is where it started" reads "$(under_cursor)" "marker here."

echo
echo "a word that turns up once"
look_for "solitary"
check "nothing is said while there is nowhere to go" none "$(noticed)"
press ctrl Down
sleep 0.4
check "asking for another one says there is only the one" some "$(noticed)"
# Undo belongs to the document. If the bar answered it, the word would come apart under
# the writer and the notice standing on it would go with it.
press ctrl z
sleep 0.4
check "ctrl+z leaves the word being looked for alone" some "$(noticed)"
type_in "x"
sleep 0.5
check "and the notice goes as soon as the word does" none "$(noticed)"
press Escape
sleep 0.3
check "a word that is not there leaves the cursor where the last one was" \
    reads "$(under_cursor)" "solitary marker word."

echo
if [ "$failures" -gt 0 ]; then
    echo "$failures failed"
    exit 1
fi
echo "all passed"
