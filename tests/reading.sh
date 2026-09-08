#!/usr/bin/env bash
# Reading mode, driven from outside the window: ctrl+r renders the document the whole way
# down, stops the checker and takes the cursor away, and the keys that move about it still
# move about it. Ctrl+r again puts the writer back where they were standing.
#
# Runs in a headless sway of its own; harness.sh has what that needs.
#
#     cargo build && tests/reading.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"

# Whether the page moved since `remember` last looked. The scrollbar shows itself while
# the view is moving and fades once it stops, so both photographs are taken from a page
# that has settled — otherwise the bar alone would answer for the page.
remember() { grim "$work/before.png"; }

moved() {
    grim "$work/after.png"
    local pixels
    pixels="$("$imagemagick" compare -metric AE "$work/before.png" "$work/after.png" \
        null: 2>&1 || true)"
    [ "${pixels%% *}" != 0 ] && return 0
    echo "the page did not move"
    return 1
}

# Enough plain prose to leave the window somewhere to scroll to, then a typo for the
# checker and a heading to stand in. Those two go at the end because that is where a file
# nobody has opened before is opened at, and it is where the cursor is wanted.
{
    for line in $(seq 30); do
        printf 'Plain prose, and nothing wrong with it, on line %s.\n\n' "$line"
    done
    printf 'I recieve mail.\n\n'
    printf '## A heading\n'
} > "$document"

start_editor

echo "the block under the cursor"
check "a heading is open to be written in" some "$(opened)"
press ctrl r
sleep 0.5
check "ctrl+r renders it like everything else" none "$(opened)"
press ctrl r
sleep 0.5
check "and ctrl+r again opens it back up" some "$(opened)"

echo
echo "the checker"
press Up
repeat 7 Left
sleep "$settle"
check "it marks the typo while the post is being written" some "$(marked)"
check "and says what it makes of it at the foot of the window" some "$(at_the_foot)"
press ctrl r
sleep "$settle"
check "reading takes the marks off the words" none "$(marked)"
check "and takes the checker off the foot of the window" none "$(at_the_foot)"

echo
echo "moving about it"
# The document is standing at its end, so every one of these goes up before it comes back.
sleep 1.5
remember
repeat 3 Up
sleep 1.5
check "the arrows move the page" moved
remember
press Next
sleep 1.5
check "and so does page down" moved
remember
press Prior
sleep 1.5
check "and page up" moved

# Typed into a document being read, and answered for by the check at the foot of this file.
type_in "not typed"
press Return
press BackSpace

echo
echo "back to the writing"
press ctrl r
sleep "$settle"
# The cursor never moved, so coming back out brings the page to it — the marks that were
# paged away from are on screen again.
check "the page comes back to the block the cursor was left in" some "$(marked)"
check "and the checker has its say again" some "$(at_the_foot)"
check "with nothing typed into the reading having been written" \
    reads "$(written)" "$(cat "$document")"

echo
if [ "$failures" -gt 0 ]; then
    echo "$failures failed"
    exit 1
fi
echo "all passed"
