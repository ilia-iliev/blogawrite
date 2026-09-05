import QtQuick
import com.blogawrite
import com.blogawrite.text

// The block under the cursor. Prose is styled as it is written, with the markers shrunk
// away; structure — headings, quotes, tables, rules — opens up into its raw markdown.
//
// The far end of a selection that runs across blocks is an editor like this one too, so
// that it can show its share of the selection. That one holds neither cursor nor keyboard.
//
// A plain TextEdit, not Controls' TextArea, which is a TextEdit with a background and a
// placeholder wrapped around it: every property this sets is one it overrides anyway, and
// naming it would load the whole of QtQuick.Controls before the first frame.
TextEdit {
    id: root

    property string source
    property string kind: "paragraph"
    property int initialPosition: -1
    // Where the block was clicked, so the cursor lands under the pointer. -1 for elsewhere.
    property point initialPoint: Qt.point(-1, -1)

    // A selection running through this block: where the rest of it lies — "above",
    // "below", "here" once it has come back inside this block, "" for none at all — and
    // where it is pinned in here, -1 when it is pinned in another block.
    property string beyond: ""
    property int anchoredAt: -1
    // Whether this is the block the cursor is in.
    property bool current: true

    // The checker's objection to what the cursor is standing in, as far as this block
    // need know it: the span a suggestion goes in — -1 when the checker offered none —
    // and the one suggestion on show. Which of them that is, the model keeps.
    property int lintAt: -1
    property int lintLength: 0
    property string lintReplacement: ""
    // The misspelled word under the cursor, where that is what the checker objected to.
    property string lintWord: ""

    // Whether the typing has stopped. Nothing is marked in a block being typed into: a
    // word half-written is not a word spelled wrong, and a sentence half-written is not
    // yet a sentence. The checker has its say once the writer pauses.
    property bool settled: true

    readonly property bool spanning: beyond === "above" || beyond === "below"
    // Where this block's own selection is pinned: the end of it the cursor is not at.
    readonly property int ownAnchor: cursorPosition === selectionStart ? selectionEnd
                                                                      : selectionStart

    // Styling the document counts as a change, so edits are only reported once the
    // block's own text is in place.
    property bool started: false
    // The text as the model last heard it. Styling counts as a change of the document,
    // and the cursor moving is enough to provoke one, so the text itself is what says
    // whether anything was actually typed.
    property string reported: ""

    readonly property bool live: kind === "paragraph" || kind === "list"
    readonly property bool code: kind === "code"

    signal edited(string body, int cursor)
    signal cursorMoved(int cursor)
    signal undoRequested()
    signal split(string before, string after)
    signal mergeRequested()
    signal leave(int direction)
    signal extend(int direction, int from)
    signal collapse(int at)
    signal selectAllRequested()
    signal tapped()
    signal copyRequested(int at)
    signal deleteRequested(int at, string insert)
    signal cycleLintRequested(int direction)
    signal learnRequested(string word)

    wrapMode: TextEdit.Wrap
    selectByMouse: true
    persistentSelection: true
    // Inverted ink rather than the stock blue, which fights the paper-coloured theme.
    // The blocks a selection covers whole are given the same ink, so that a selection
    // across several of them is all one colour.
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

    // TextArea's `background`, as the child it would have been: behind the text, and
    // sized to the editor rather than to itself.
    Rectangle {
        z: -1
        visible: !root.live
        width: root.width
        y: root.code ? -6 : 0
        height: root.height + (root.code ? 12 : 0)
        color: root.code ? Theme.codeBackground : Theme.activeBackground
        border.color: Theme.border
        radius: 3
    }

    MarkdownHighlighter {
        // A raw block is left alone: its markup is the point of showing it.
        target: root.live || root.code ? root.textDocument : null
        // Markers open up around the cursor, and there is none in a block it has left.
        cursorPosition: root.current ? root.cursorPosition : -1
        code: root.code
        accent: Theme.accent
        muted: Theme.muted
        codeBackground: Theme.codeBackground
        lint: Theme.lint
        settled: root.settled
        monoFamily: Theme.monoFamily
        codeSize: Theme.codeSize
        lineHeight: Theme.lineHeight
    }

    // A click brings the cursor here, and with it the end of any selection that ran
    // past this block.
    TapHandler {
        onTapped: root.tapped()
    }

    Component.onCompleted: {
        reported = source
        text = source
        started = true
        // The model hears about `settled` when it changes, and a fresh editor is never
        // mid-word. Without saying so, a block reached while another was being typed in
        // would show its marks with nothing at the foot of the window to go with them.
        settledChanged()
        if (current) {
            place()
            settle.start()
        } else {
            showSelection()
        }
    }

    // How long a pause counts as having stopped typing. Long enough to type through the
    // end of a word and into the next one, short enough that a writer who paused to
    // think finds the checker has already caught up.
    Timer {
        id: pause

        interval: 600
        onTriggered: root.settled = true
    }

    // The loader sizes the editor only after it is built, so the click lands properly on
    // the second go; on the very first block the window is not up for focus either. A
    // timer rather than Qt.callLater, which would fire into a block already gone.
    Timer {
        id: settle

        interval: 0
        onTriggered: {
            if (root.current) {
                root.place()
            }
        }
    }

    // The model rewrites this block under the editor when a selection is deleted across it.
    onSourceChanged: {
        if (started && text !== source) {
            reported = source
            text = source
            cursorPosition = initialPosition < 0 ? length : Math.min(initialPosition, length)
        }
    }

    onCurrentChanged: {
        if (!current) {
            showSelection()
        } else if (!activeFocus) {
            // The cursor has come back to this block from another one. A click brings it
            // back too, and has already put it where it belongs.
            place()
        }
    }

    onBeyondChanged: {
        if (beyond !== "") {
            showSelection()
        } else if (!current) {
            // A selection let go leaves nothing behind in the block it ran into. The
            // one with the cursor keeps its own, which the editor sees to from here.
            deselect()
        }
    }

    // Put the cursor where the block was clicked, or where the last block left it.
    function place() {
        const at = initialPoint.x >= 0
            ? positionAt(initialPoint.x, initialPoint.y)
            : initialPosition < 0 ? length : Math.min(initialPosition, length)
        cursorPosition = code ? insideFences(at) : at
        showSelection()
        forceActiveFocus()
    }

    // This block's share of a selection: from where the selection is pinned — in here, or
    // at the edge it comes in by — to the cursor, or to the edge it leaves by.
    function showSelection() {
        if (beyond === "") {
            return
        }
        const pinned = anchoredAt >= 0 ? Math.min(anchoredAt, length)
                     : beyond === "above" ? 0 : length
        const far = current ? cursorPosition : (beyond === "above" ? 0 : length)
        select(pinned, far)
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

    onCursorPositionChanged: {
        if (started && current) {
            root.cursorMoved(cursorPosition)
        }
    }

    onTextChanged: {
        if (!started || text === reported) {
            return
        }
        reported = text
        root.settled = false
        pause.restart()
        root.edited(text, cursorPosition)
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
        // Tildes have no such arithmetic — a pair is a strikeout — but the same count answers.
        const mark = marker.charAt(0)
        const run = markerRun(mark, start, -1)
        const marked = run === markerRun(mark, end, 1)
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

    // How many `mark`s are packed against `position`, looking back (side -1) or on (side 1).
    function markerRun(mark, position, side) {
        let run = 0
        while (text.charAt(side < 0 ? position - run - 1 : position + run) === mark) {
            run += 1
        }
        return run
    }

    // Put the suggestion on show where the checker objected, leaving the cursor at the
    // end of it. Written as one edit rather than a remove and an insert, so that one
    // undo takes it back; the model hears about it the way it hears about typing.
    // Taken down first: the edit goes to the model, which looks at the block again and
    // has nothing left to object to, and the properties below are gone by the next line.
    function acceptLint() {
        if (lintAt < 0) {
            return
        }
        const at = lintAt
        const replacement = lintReplacement
        text = text.slice(0, at) + replacement + text.slice(at + lintLength)
        cursorPosition = at + replacement.length
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

    // The keys that go on moving the far end of a selection rather than ending it: the
    // cursor keys, with shift. Within this block the editor moves it itself.
    function movesSelection(event) {
        if (!(event.modifiers & Qt.ShiftModifier)) {
            return false
        }
        switch (event.key) {
        case Qt.Key_Up:
        case Qt.Key_Down:
        case Qt.Key_Left:
        case Qt.Key_Right:
        case Qt.Key_Home:
        case Qt.Key_End:
            return true
        }
        return false
    }

    // What a selection that runs past this block answers itself. Everything else ends it
    // and then does its usual job.
    function takesSelection(event) {
        switch (event.key) {
        case Qt.Key_C:
            if (event.modifiers === Qt.ControlModifier) {
                root.copyRequested(cursorPosition)
                return true
            }
            return false
        case Qt.Key_Backspace:
        case Qt.Key_Delete:
        case Qt.Key_Return:
        case Qt.Key_Enter:
            root.deleteRequested(cursorPosition, "")
            return true
        case Qt.Key_Shift:
        case Qt.Key_Control:
            // A modifier on its own is the start of one of these, not the end of them.
            return true
        }
        // Anything typed replaces the selection, as it does in any editor.
        if (event.text.length > 0 && event.text.charCodeAt(0) >= 0x20
                && !(event.modifiers & Qt.ControlModifier)) {
            root.deleteRequested(cursorPosition, event.text)
            return true
        }
        return false
    }

    Keys.onPressed: (event) => {
        // Select-all reaches past this block, so the document answers it before any
        // selection already running is let go: it replaces that one rather than ending it.
        if (event.key === Qt.Key_A && event.modifiers === Qt.ControlModifier) {
            event.accepted = true
            root.selectAllRequested()
            return
        }

        // A selection that is running answers some keys itself, the cursor keys carry
        // on moving it, and everything else lets it go before doing its usual job.
        if (beyond !== "" && !movesSelection(event)) {
            if (spanning && takesSelection(event)) {
                event.accepted = true
                return
            }
            root.collapse(cursorPosition)
        }

        switch (event.key) {
        case Qt.Key_Z:
            if (event.modifiers === Qt.ControlModifier) {
                event.accepted = true
                root.undoRequested()
            }
            break
        case Qt.Key_Backspace:
            if (cursorPosition === 0 && selectedText.length === 0) {
                event.accepted = true
                root.mergeRequested()
            }
            break
        case Qt.Key_Up:
        case Qt.Key_Down:
            let direction = event.key === Qt.Key_Down ? 1 : -1
            // Control walks the suggestions the checker offered rather than the text.
            // Where it has offered none it is left to move the cursor as it always did.
            if (event.modifiers === Qt.ControlModifier) {
                if (lintAt >= 0) {
                    event.accepted = true
                    root.cycleLintRequested(direction)
                }
                break
            }
            let atEdge = direction > 0
                ? cursorRectangle.y >= positionToRectangle(length).y
                : cursorRectangle.y <= positionToRectangle(0).y
            if (atEdge) {
                event.accepted = true
                // Shift at the edge carries the selection on into the next block instead
                // of stopping at this one's.
                if (event.modifiers & Qt.ShiftModifier) {
                    root.extend(direction, ownAnchor)
                } else {
                    root.leave(direction)
                }
            }
            break
        case Qt.Key_Left:
        case Qt.Key_Right:
            // Only with shift: the plain arrows have always stopped at a block's ends.
            let step = event.key === Qt.Key_Right ? 1 : -1
            if ((event.modifiers & Qt.ShiftModifier)
                    && cursorPosition === (step > 0 ? length : 0)) {
                event.accepted = true
                root.extend(step, ownAnchor)
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
        case Qt.Key_U:
            if (event.modifiers === Qt.ControlModifier) {
                event.accepted = true
                root.surround("~~")
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
            if (event.modifiers === Qt.ControlModifier) {
                if (lintAt >= 0) {
                    event.accepted = true
                    root.acceptLint()
                }
                break
            }
            // Shift as well: the word is spelled the way the writer meant it, and the
            // dictionary is the one that is wrong. It keeps the word from here on.
            if (event.modifiers === (Qt.ControlModifier | Qt.ShiftModifier)) {
                if (lintWord !== "") {
                    event.accepted = true
                    root.learnRequested(lintWord)
                }
                break
            }
            // A second Enter ends the block rather than adding a blank line to it.
            if (cursorPosition > 0 && text.charAt(cursorPosition - 1) === "\n") {
                event.accepted = true
                root.split(text.slice(0, cursorPosition - 1), text.slice(cursorPosition))
            }
            break
        }
    }
}
