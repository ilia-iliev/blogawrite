import QtQuick
import com.blogawrite
import com.blogawrite.text

// Nothing here comes from QtQuick.Controls. It would be worth about twenty milliseconds
// of every launch — two libraries and two QML modules, loaded before the first frame —
// and this window wants three things from it: a window, a scrollbar and a prompt.
Window {
    id: window

    width: 960
    height: 760
    visible: true
    color: Theme.background
    title: (doc.dirty ? "● " : "") + fileName + " — blogawrite"

    readonly property string fileName: doc.filePath.slice(doc.filePath.lastIndexOf("/") + 1)

    Document {
        id: doc
    }

    // The checker takes a moment longer to come up than the window does, and a word taken
    // into the dictionary changes what it would say about every block at once. The blocks
    // are told to look again by the highlighters themselves; the foot of the window, here.
    Connections {
        target: CheckerWatch
        function onChanged() {
            doc.refreshLint()
        }
    }

    // main.rs has already refused to start without a file, so there is always one here.
    Component.onCompleted: {
        if (!doc.openPath(Qt.application.arguments[1])) {
            Qt.exit(1)
        }
    }

    ListView {
        id: view

        anchors.fill: parent
        topMargin: 40
        bottomMargin: 200
        model: doc
        spacing: Theme.blockSpacing
        cacheBuffer: 800
        boundsBehavior: Flickable.StopAtBounds

        delegate: Block {
            document: doc
            onCopyRequested: (body) => clipboard.take(body)
            onSearchRequested: search.open()
        }

        // Hand the keyboard back to the block being edited. A Controls Popup returned it
        // by itself; this one has to be asked.
        function refocusActive() {
            // There is no editor to hand it back to while the document is being read;
            // the keys that move about it are answered outside the view instead.
            if (doc.reading) {
                reader.forceActiveFocus()
                return
            }
            const item = view.itemAtIndex(doc.activeIndex)
            if (item) {
                item.refocus()
            }
        }

        // Keep the edited block on screen without letting the view claim keyboard focus.
        // Deferred so it also works before the first layout, on startup.
        function showActive() {
            if (doc.activeIndex < 0) {
                return
            }
            // A block taller than the window is scrolled through by hand; don't fight that.
            if (view.scrollingWithin(view.itemAtIndex(doc.activeIndex))) {
                return
            }
            view.positionViewAtIndex(doc.activeIndex, ListView.Contain)
            // Contain leaves the block flush with the window edge; lift it clear of the fade.
            const item = view.itemAtIndex(doc.activeIndex)
            if (item) {
                const overlap = item.y + item.height + 48 - (view.contentY + view.height)
                if (overlap > 0) {
                    view.contentY += overlap
                }
            }
        }

        // Page through the document a screenful at a time. The block at the far edge stays
        // selected and lands on the near edge, so nothing between the two screenfuls is missed.
        function page(direction) {
            // Reading has no cursor to carry along, so the view is all that moves.
            if (doc.reading) {
                view.scroll(direction * view.height * 0.9)
                return
            }

            const anchor = view.edgeIndex(direction)
            if (anchor < 0) {
                return
            }
            doc.activate(anchor)

            const before = view.contentY
            view.positionViewAtIndex(doc.activeIndex,
                                     direction > 0 ? ListView.Beginning : ListView.End)
            // A block taller than the window has no far edge to travel to: scroll inside it.
            if (direction * (view.contentY - before) <= 0) {
                view.contentY = before
                view.scroll(direction * view.height * 0.9)
            }
        }

        // Move the view by `distance`, stopping at the ends of the document.
        function scroll(distance) {
            view.contentY += distance
            view.returnToBounds()
        }

        function scrollingWithin(item) {
            return item && item.height > view.height
                && item.y < view.contentY + view.height && item.y + item.height > view.contentY
        }

        // The last block still on screen in `direction`.
        function edgeIndex(direction) {
            const edge = direction > 0 ? view.contentY + view.height - 1 : view.contentY
            // Step inwards: the edge itself can fall in the gap between two blocks.
            for (let inset = 0; inset < view.height; inset += 4) {
                const found = view.indexAt(view.width / 2, edge - direction * inset)
                if (found >= 0) {
                    return found
                }
            }
            return -1
        }

        // Images report their size only once loaded, which shifts everything below them,
        // so look again after the dust has settled.
        Timer {
            id: settle
            interval: 400
            onTriggered: view.showActive()
        }

        Connections {
            target: doc
            function onActiveIndexChanged() {
                Qt.callLater(view.showActive)
                settle.restart()
            }

            // Reading moves the view and not the cursor, so coming back out brings the
            // view back to it — and the editor that opens there takes the keyboard.
            function onReadingChanged() {
                if (!doc.reading) {
                    Qt.callLater(view.showActive)
                }
            }
        }
    }

    // Where you are in the document, shown while it is being scrolled and fading out
    // once it settles — the job Controls' ScrollBar was doing, minus the dragging,
    // which this never needed.
    Rectangle {
        id: scrollBar

        anchors.right: parent.right
        anchors.rightMargin: 2
        width: 6
        radius: 3
        color: Theme.muted
        visible: view.contentHeight > view.height
        y: view.visibleArea.yPosition * view.height
        height: Math.max(24, view.visibleArea.heightRatio * view.height)
        opacity: 0

        // A wheel notch moves the view and is over with; the bar has to outlast it, or
        // it would show for the single frame the movement took.
        Timer {
            id: linger
            interval: 700
        }

        Connections {
            target: view
            function onContentYChanged() {
                linger.restart()
            }
        }

        // There the moment you scroll, gone gently once you stop — so the transition
        // is on the way out only.
        states: State {
            name: "scrolling"
            when: linger.running
            PropertyChanges {
                target: scrollBar
                opacity: 0.4
            }
        }

        transitions: Transition {
            to: ""
            NumberAnimation { property: "opacity"; duration: 400 }
        }
    }

    // Reading mode has no editor to hold the keyboard, so the keys that move about the
    // document are answered here. The blocks below are rendered and take none of them.
    Item {
        id: reader

        anchors.fill: parent
        focus: doc.reading

        // One line of prose: what an arrow key moves the page by.
        readonly property real step: Theme.bodySize * Theme.lineHeight

        Keys.onPressed: (event) => {
            switch (event.key) {
            case Qt.Key_R:
                if (event.modifiers === Qt.ControlModifier) {
                    event.accepted = true
                    doc.toggleReading()
                }
                break
            case Qt.Key_Up:
            case Qt.Key_Down:
                event.accepted = true
                view.scroll(event.key === Qt.Key_Down ? reader.step : -reader.step)
                break
            }
        }
    }

    // Which of the two things the window is doing that it would otherwise not show. A
    // screenful with no cursor in it looks the same read as written — every block but
    // one is rendered either way — and a key typed into reading mode does nothing. The
    // checker turned off is worth saying for as long as it is off for its own reason:
    // on a paragraph with nothing wrong in it, the marks going away is no answer at all.
    Text {
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 12
        visible: text !== ""
        text: doc.reading ? qsTr("reading") : doc.checking ? "" : qsTr("checking off")
        color: Theme.muted
        font.family: Theme.bodyFamily
        font.pixelSize: Theme.bodySize
    }

    // Content scrolls under the window edge; fade it out rather than cutting it dead.
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 36
        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.alpha(Theme.background, 0) }
            GradientStop { position: 1.0; color: Theme.background }
        }
    }

    // File errors must be visible when the app was launched from a desktop entry, where
    // stderr has nowhere useful to go.
    FootBar {
        visible: doc.errorMessage !== ""
        z: 2

        Text {
            width: parent.width
            text: doc.errorMessage
            wrapMode: Text.WordWrap
            color: Theme.promptText
            font.family: Theme.bodyFamily
            font.pixelSize: Theme.bodySize
        }
    }

    // What the checker makes of where the cursor is standing, at the foot of the window.
    // The wash on the words says that something is wrong with them; this says what.
    FootBar {
        visible: doc.lintMessage !== "" && !doc.searchActive

        Text {
            width: parent.width
            // The checker writes its messages in markdown, with the words it is talking
            // about in backticks, and the suggestion is put in the same voice. One with
            // nothing in it is a suggestion to take the words out.
            readonly property string suggestion:
                doc.lintOptions === 0 ? ""
              : doc.lintReplacement === "" ? "  →  *delete*"
              : "  →  `" + doc.lintReplacement + "`"
            // Only ever one suggestion on show. The count is there to say that ctrl with
            // the up and down keys has somewhere to go.
            readonly property string counter:
                doc.lintOptions > 1 ? "  " + (doc.lintChoice + 1) + "/" + doc.lintOptions : ""

            text: doc.lintMessage + suggestion + counter
            textFormat: Text.MarkdownText
            wrapMode: Text.WordWrap
            color: Theme.promptText
            font.family: Theme.bodyFamily
            font.pixelSize: Theme.bodySize
        }
    }

    // Qt hands the clipboard to text items alone, so a selection that spans blocks is
    // copied by way of one that exists for nothing else.
    TextEdit {
        id: clipboard

        visible: false

        function take(body) {
            text = body
            selectAll()
            copy()
        }
    }

    SearchBar {
        id: search

        document: doc
        onClosed: view.refocusActive()
    }

    Shortcut { sequences: [StandardKey.Save]; onActivated: doc.save() }
    Shortcut { sequences: [StandardKey.MoveToNextPage]; onActivated: view.page(1) }
    Shortcut { sequences: [StandardKey.MoveToPreviousPage]; onActivated: view.page(-1) }

    onClosing: (close) => {
        doc.closeSearch()
        doc.rememberPosition()
        if (doc.dirty) {
            close.accepted = false
            closePrompt.open()
        }
    }

    ClosePrompt {
        id: closePrompt

        document: doc
        onCancelled: view.refocusActive()
    }
}
