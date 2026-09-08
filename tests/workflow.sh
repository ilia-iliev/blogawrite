#!/usr/bin/env bash
# What the checker is for, driven from outside the window: type something wrong, pause,
# watch it get marked, walk what is offered, take one. A typo and a clumsy turn of phrase
# go through the very same steps, which is the point of the exercise — so both are typed,
# and both are asked for the same things. Then it is switched off, and says nothing at all.
#
# Runs in a headless sway of its own; harness.sh has what that needs. Ctrl+S wants a window
# the compositor has made active, so the text is read back off the clipboard instead, which
# the editor puts there itself.
#
#     cargo build && tests/workflow.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/harness.sh"

# An empty document: everything this test is about is typed into it.
printf '' > "$document"
start_editor

echo "a typo"
type_in "I recieve mail"
check "nothing is marked while the typing is still going on" none "$(marked)"
sleep "$settle"
check "the typo is marked once the typing stops" some "$(marked)"
check "and nothing is offered until the cursor stands in it" none "$(at_the_foot)"

# Back into the word: the checker offers what to do about the words the cursor is in.
# Straight away — the pause is for typing, and moving the cursor is not typing.
repeat 5 Left
sleep 0.4
check "standing in it offers what was meant instead" some "$(at_the_foot)"

press ctrl Return
sleep "$settle"
check "accepting puts the dictionary's word in" reads "$(written)" "I receive mail"
check "and there is nothing left to mark" none "$(marked)"

echo
echo "a turn of phrase"
type_in ". This is very unique"
sleep "$settle"
check "the phrase is marked the same way" some "$(marked)"
check "and offered the same way" some "$(at_the_foot)"

# Three forward and two back is one forward, however many the checker offered, and its
# second thought about `very unique` is `very rare`.
repeat 3 ctrl Down
repeat 2 ctrl Up
press ctrl Return
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

press ctrl shift Return
sleep "$settle"
check "keeping it takes the mark off" none "$(marked)"
check "and writes it down for next time" \
    reads "$(cat "$work/config/blogawrite/dictionary" 2>/dev/null)" "Blogawrite"

echo
echo "a table"
repeat 2 Return
type_in "| Naem | Vlaue |"
press Return
type_in "| --- | --- |"
press Return
type_in "| tpyo | anohter |"
sleep "$settle"
check "a table is a table, not prose" none "$(marked)"

echo
echo "the checker turned off"
repeat 2 Return
type_in "I recieve mail"
repeat 5 Left
sleep "$settle"
check "a typo is marked and offered as ever" some "$(marked)"
press ctrl g
sleep "$settle"
check "turning the checking off takes the marks off the words" none "$(marked)"
check "and takes what was offered off the foot of the window" none "$(at_the_foot)"
press ctrl g
sleep "$settle"
check "turning it back on marks it again" some "$(marked)"
check "and offers what was meant again" some "$(at_the_foot)"

echo
if [ "$failures" -gt 0 ]; then
    echo "$failures failed"
    exit 1
fi
echo "all passed"
