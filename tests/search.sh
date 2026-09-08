#!/usr/bin/env bash
# What the search is for, driven from outside the window: open it, type a word, watch the
# cursor land on it, walk the occurrences round the document and back, and close it again.
# Which occurrence the cursor is standing on is read off the clipboard — the selection is
# stretched to the end of its line first, so that three occurrences of the same word are
# told apart by what follows them.
#
# Runs in a headless sway of its own; harness.sh has what that needs.
#
#     cargo build && tests/search.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"

# Nothing in the document below is drawn in either of the two colours this counts — it is
# plain prose, with no markup for the accent to land on — so the foot of the window is
# empty until the search bar fills it, and the accent turns up only under the notice.
#
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

# Three occurrences of one word, one to a block, each with something different after it;
# and a word that turns up once. Plain prose throughout, so the checker has nothing to say
# and the foot of the window stays empty until the search is opened.
cat > "$document" <<'DOCUMENT'
One marker here.

Two marker there.

A solitary marker word.
DOCUMENT

start_editor

echo "opening and closing"
check "the foot of the window is empty to begin with" none "$(at_the_foot)"
press ctrl f
sleep 0.4
check "ctrl+f brings the search bar up" some "$(at_the_foot)"
press ctrl f
sleep 0.4
check "and ctrl+f again takes it down" none "$(at_the_foot)"
press ctrl f
sleep 0.4
press Escape
sleep 0.4
check "so does escape" none "$(at_the_foot)"
press ctrl f
sleep 0.4
press Return
sleep 0.4
check "and so does enter, the word being typed" none "$(at_the_foot)"

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
repeat 2 ctrl Down
press Escape
sleep 0.3
check "and on to the one after that" reads "$(under_cursor)" "marker word."

look_for "marker"
repeat 3 ctrl Down
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
