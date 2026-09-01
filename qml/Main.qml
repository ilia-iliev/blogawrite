import QtQuick
import QtQuick.Controls.Basic
import com.blogawrite

ApplicationWindow {
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

        ScrollBar.vertical: ScrollBar {}

        delegate: Item {
            id: block

            required property int index
            required property string text
            required property string kind
            required property string imagePath

            readonly property bool editing: index === doc.activeIndex
            // Where this block was last clicked, handed to the editor it opens.
            property point tapPoint: Qt.point(-1, -1)

            width: ListView.view.width
            height: column.height

            onEditingChanged: {
                if (!editing) {
                    tapPoint = Qt.point(-1, -1)
                }
            }

            Column {
                id: column

                x: (parent.width - width) / 2
                width: Math.min(Theme.contentWidth, parent.width - 64)
                spacing: 8

                // A lone image keeps its picture even while its markdown is being edited.
                Loader {
                    width: parent.width
                    height: item ? item.implicitHeight : 0
                    active: block.kind === "image"
                    sourceComponent: imageBlock
                }

                Loader {
                    id: loader

                    width: parent.width
                    height: item ? item.implicitHeight : 0
                    active: block.editing || block.kind !== "image"
                    sourceComponent: block.editing ? activeBlock : renderedBlock
                }
            }

            Component {
                id: renderedBlock
                RenderedBlock {
                    source: block.text
                    documentBase: doc.baseUrl
                    onActivated: (at) => {
                        block.tapPoint = at
                        doc.activate(block.index)
                    }
                }
            }

            Component {
                id: imageBlock
                ImageBlock {
                    imagePath: block.imagePath
                    documentBase: doc.baseUrl
                    onActivated: doc.activate(block.index)
                }
            }

            Component {
                id: activeBlock
                ActiveBlock {
                    source: block.text
                    kind: block.kind
                    initialPosition: doc.pendingCursor
                    initialPoint: block.tapPoint
                    onEdited: (body) => doc.setBlockText(block.index, body)
                    onSplit: (before, after) => doc.splitBlock(block.index, before, after)
                    onMergeRequested: doc.mergeWithPrevious(block.index)
                    onLeave: (direction) => doc.activate(block.index + direction)
                }
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
                view.contentY = before + direction * view.height * 0.9
                view.returnToBounds()
            }
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
        }
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

    Shortcut { sequence: StandardKey.Save; onActivated: doc.save() }
    Shortcut { sequence: StandardKey.MoveToNextPage; onActivated: view.page(1) }
    Shortcut { sequence: StandardKey.MoveToPreviousPage; onActivated: view.page(-1) }

    onClosing: (close) => {
        doc.rememberPosition()
        if (doc.dirty) {
            close.accepted = false
            closePrompt.open()
        }
    }

    // A prompt line at the foot of the window rather than a box of buttons.
    Popup {
        id: closePrompt

        parent: window.contentItem
        width: parent.width
        y: parent.height - height
        padding: 12
        modal: true
        dim: false
        focus: true

        // It is answered by keystroke, so it has to hold the keyboard itself.
        onOpened: prompt.forceActiveFocus()

        background: Rectangle {
            color: Theme.activeBackground
            border.color: Theme.border
        }

        contentItem: Row {
            id: prompt

            spacing: 16

            Text {
                text: qsTr("Save changes to %1?").arg(window.fileName)
                color: Theme.text
                font.family: Theme.monoFamily
                font.pixelSize: Theme.bodySize
            }

            Text {
                text: qsTr("[y] save   [n] discard   [esc] cancel")
                color: Theme.muted
                font.family: Theme.monoFamily
                font.pixelSize: Theme.bodySize
            }

            Keys.onPressed: (event) => {
                event.accepted = true
                switch (event.key) {
                case Qt.Key_Y:
                case Qt.Key_Return:
                case Qt.Key_Enter:
                    // A failed save keeps the window open rather than losing the text.
                    if (doc.save()) {
                        Qt.quit()
                    }
                    break
                case Qt.Key_N:
                    // Nothing is written; clearing the flag just lets the close through.
                    doc.dirty = false
                    Qt.quit()
                    break
                case Qt.Key_Escape:
                    closePrompt.close()
                    break
                }
            }
        }
    }
}
