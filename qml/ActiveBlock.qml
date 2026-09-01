import QtQuick
import QtQuick.Controls.Basic
import com.blogawrite
import com.blogawrite.text

// The block under the cursor. Prose is styled as it is written, with the markers shrunk
// away; structure — headings, quotes, tables, rules — opens up into its raw markdown.
TextArea {
    id: root

    property string source
    property string kind: "paragraph"
    property int initialPosition: -1
    // Where the block was clicked, so the cursor lands under the pointer. -1 for elsewhere.
    property point initialPoint: Qt.point(-1, -1)

    // Styling the document counts as a change, so edits are only reported once the
    // block's own text is in place.
    property bool started: false

    readonly property bool live: kind === "paragraph" || kind === "list"
    readonly property bool code: kind === "code"

    signal edited(string body)
    signal split(string before, string after)
    signal mergeRequested()
    signal leave(int direction)

    wrapMode: TextArea.Wrap
    selectByMouse: true
    persistentSelection: true
    // Inverted ink rather than the stock blue, which fights the paper-coloured theme.
    selectionColor: Theme.text
    selectedTextColor: Theme.background
    color: Theme.text
    font.family: live ? Theme.bodyFamily : Theme.monoFamily
    font.pixelSize: code ? Theme.codeSize : Theme.bodySize
    // Prose sits exactly where its rendered self did; a raw block gets its box back.
    padding: live ? 0 : 8
    // Qt renders code flush with the top of its block, so the code keeps its place and
    // the box below grows upwards instead.
    topPadding: code ? 0 : padding

    background: Rectangle {
        visible: !root.live
        y: root.code ? -6 : 0
        height: root.height + (root.code ? 12 : 0)
        color: root.code ? Theme.codeBackground : Theme.activeBackground
        border.color: Theme.border
        radius: 3
    }

    MarkdownHighlighter {
        // A raw block is left alone: its markup is the point of showing it.
        target: root.live || root.code ? root.textDocument : null
        cursorPosition: root.cursorPosition
        code: root.code
        accent: Theme.accent
        muted: Theme.muted
        codeBackground: Theme.codeBackground
        monoFamily: Theme.monoFamily
        codeSize: Theme.codeSize
        lineHeight: Theme.lineHeight
    }

    Component.onCompleted: {
        text = source
        started = true
        place()
        // The loader sizes the editor only after it is built, so the click lands properly
        // on the second go; on the very first block the window is not up for focus either.
        Qt.callLater(place)
    }

    // Put the cursor where the block was clicked, or where the last block left it.
    function place() {
        const at = initialPoint.x >= 0
            ? positionAt(initialPoint.x, initialPoint.y)
            : initialPosition < 0 ? length : Math.min(initialPosition, length)
        cursorPosition = code ? insideFences(at) : at
        forceActiveFocus()
    }

    // Qt lays a rendered code block out with margins the raw text has not, so a click on it
    // can land on a fence and open the block up. Keep the cursor in the code it aimed at.
    function insideFences(position) {
        if (!text.startsWith("```") && !text.startsWith("~~~")) {
            return position
        }
        const first = text.indexOf("\n") + 1
        const last = text.lastIndexOf("\n")
        return last >= first && first > 0 ? Math.min(Math.max(position, first), last) : position
    }

    onTextChanged: {
        if (started) {
            root.edited(text)
        }
    }

    // Put `marker` either side of the selection, or start an empty pair to type into.
    function surround(marker) {
        let start = selectionStart
        let end = selectionEnd
        // Markdown wants the markers snug against the words: `**bold** `, never `**bold **`.
        while (end > start && /\s/.test(text.charAt(end - 1))) {
            end -= 1
        }
        while (start < end && /\s/.test(text.charAt(start))) {
            start += 1
        }
        // Stars work as bits: one is italic, two is bold, three is both. Only this marker's
        // own stars come off, so bold nests inside italic and a second press unpicks it.
        const run = starRun(start, -1)
        const marked = run === starRun(end, 1)
                    && (marker.length === 1 ? run % 2 === 1 : run >= 2)
        if (marked) {
            remove(end, end + marker.length)
            remove(start - marker.length, start)
            select(start - marker.length, end - marker.length)
            return
        }
        insert(end, marker)
        insert(start, marker)
        select(start + marker.length, end + marker.length)
    }

    // How many stars are packed against `position`, looking back (side -1) or on (side 1).
    function starRun(position, side) {
        let run = 0
        while (text.charAt(side < 0 ? position - run - 1 : position + run) === "*") {
            run += 1
        }
        return run
    }

    // `[selection](|)`, or `[|]()` with nothing selected. `prefix` is "!" for an image.
    function insertLink(prefix) {
        const start = selectionStart
        const end = selectionEnd
        insert(end, "]()")
        insert(start, prefix + "[")
        cursorPosition = start === end ? start + prefix.length + 1
                                       : end + prefix.length + 3
    }

    Keys.onPressed: (event) => {
        switch (event.key) {
        case Qt.Key_Backspace:
            if (cursorPosition === 0 && selectedText.length === 0) {
                event.accepted = true
                root.mergeRequested()
            }
            break
        case Qt.Key_Up:
            if (cursorRectangle.y <= positionToRectangle(0).y) {
                event.accepted = true
                root.leave(-1)
            }
            break
        case Qt.Key_Down:
            if (cursorRectangle.y >= positionToRectangle(length).y) {
                event.accepted = true
                root.leave(1)
            }
            break
        case Qt.Key_B:
            if (event.modifiers === Qt.ControlModifier) {
                event.accepted = true
                root.surround("**")
            }
            break
        case Qt.Key_I:
            if (event.modifiers === Qt.ControlModifier) {
                event.accepted = true
                root.surround("*")
            } else if (event.modifiers === (Qt.ControlModifier | Qt.ShiftModifier)) {
                event.accepted = true
                root.insertLink("!")
            }
            break
        case Qt.Key_L:
            if (event.modifiers === (Qt.ControlModifier | Qt.ShiftModifier)) {
                event.accepted = true
                root.insertLink("")
            }
            break
        case Qt.Key_Return:
        case Qt.Key_Enter:
            // A second Enter ends the block rather than adding a blank line to it.
            if (cursorPosition > 0 && text.charAt(cursorPosition - 1) === "\n") {
                event.accepted = true
                root.split(text.slice(0, cursorPosition - 1), text.slice(cursorPosition))
            }
            break
        }
    }
}
