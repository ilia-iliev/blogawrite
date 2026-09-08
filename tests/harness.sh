# The part of a test driven from outside the window that is not about what is being
# tested: a compositor of its own to run the editor in, a keyboard to type at it with, and
# a way to count what it painted. Sourced by the tests, never run on its own.
#
# Needs sway, wtype, grim, wl-clipboard and ImageMagick. The editor is the only window
# there is: sway tiles it over the whole output, and that output is what gets photographed.
# Nothing of the writer's own is touched — its home, its dictionary and where it keeps the
# cursor are all under a working directory of the moment, and the compositor draws into
# memory rather than onto a screen or a card.

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly root
readonly editor="${1:-$root/target/debug/blogawrite}"
readonly work="$(mktemp -d)"
readonly document="$work/document.md"

failures=0

cleanup() {
    kill "${app:-}" 2>/dev/null || true
    kill "${compositor:-}" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT

# One colour of the palette, read from where it is written down.
colour() { grep -oP "$1: QString::from\(\"\\K#[0-9A-Fa-f]{6}" "$root/src/theme.rs"; }

# ImageMagick 7 renamed `convert` to `magick` and warns on every use of the old name.
readonly imagemagick="$(command -v magick || command -v convert)"

# How many pixels of the output are painted in `$1`.
painted() {
    grim "$work/shot.png"
    "$imagemagick" "$work/shot.png" -format %c histogram:info:- \
        | awk -F: -v colour="$1" '$0 ~ colour { gsub(/ /, "", $1); print $1; found = 1; exit }
                                  END { if (!found) print 0 }'
}

# The colours worth counting, each read from where the palette is written down: the wash
# behind anything the checker objects to, the dark ground of the bars across the foot of
# the window, the box a block opens up into when the cursor is in it, and the accent an
# answer of the editor's own is put on. What a count of them means is the business of the
# check that asks for it.
readonly wash="$(colour lint)"
readonly foot="$(colour prompt_background)"
readonly box="$(colour active_background)"
readonly accent="$(colour accent)"

marked() { painted "$wash"; }
at_the_foot() { painted "$foot"; }
opened() { painted "$box"; }
noticed() { painted "$accent"; }

# Long enough for the checker to have had its say: the editor waits 600ms of quiet before
# it says anything, and then has to draw it.
readonly settle=1.5

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

# The same keystroke `$1` times over, for the keys a writer holds down: `repeat 5 Left`.
repeat() {
    local times="$1"
    shift
    local _
    for _ in $(seq "$times"); do
        press "$@"
    done
}

# What the editor holds, by way of its own copy: select the document, copy it, put the
# cursor back where the text ends.
written() {
    press ctrl a
    press ctrl c
    sleep 0.4
    press Right
    wl-paste --no-newline 2>/dev/null
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

# Bring the compositor up and start the editor in it on the document the test wrote.
start_editor() {
    mkdir -p "$work/home" "$work/run"
    chmod 700 "$work/run"
    export XDG_RUNTIME_DIR="$work/run"
    export WLR_BACKENDS=headless
    export WLR_RENDERER=pixman
    export WLR_LIBINPUT_NO_DEVICES=1
    # Qt takes a Wayland session over an X one wherever it is offered, and either of these
    # left standing would put the window on the writer's own screen instead of ours.
    unset WAYLAND_DISPLAY DISPLAY

    cat > "$work/sway.conf" <<'CONFIG'
output HEADLESS-1 resolution 1200x900
default_border none
default_floating_border none
CONFIG

    sway -c "$work/sway.conf" >"$work/sway.log" 2>&1 &
    compositor=$!
    # Its socket is the only one under our own runtime directory, so it says when it is up.
    local socket=""
    local candidate
    local _
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
    # The checker's rules and dictionaries take the better part of a second to load, and
    # nothing it has to say is on screen before they are there.
    sleep 3
}
