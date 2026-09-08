import QtQuick
import com.blogawrite

// Find a word in the document. The bar holds the keyboard while it is open — the writer
// is still typing what they are looking for — and each occurrence it walks to is selected
// by the cursor in the text above. Opened from the block being edited, which sees the
// keystroke first; closed from in here.
FootBar {
    id: root

    property Document document

    // The keyboard has to go back to the block being edited, which only the window knows.
    signal closed()

    visible: document.searchActive
    // Above the block being edited and above a file error, which is what the writer
    // asked for by opening this; both are back the moment it closes.
    z: 3

    function open() {
        document.openSearch()
        needle.forceActiveFocus()
        // Re-opened on the word looked for last time: it is found again straight
        // away, and typing replaces it rather than adding to it.
        needle.selectAll()
        document.searchFor(needle.text)
    }

    function close() {
        document.closeSearch()
        root.closed()
    }

    Item {
        id: bar

        width: parent.width
        height: needle.height

        Text {
            id: label

            anchors.verticalCenter: parent.verticalCenter
            text: qsTr("find")
            color: Theme.muted
            font.family: Theme.monoFamily
            font.pixelSize: Theme.bodySize
        }

        TextInput {
            id: needle

            anchors.left: label.right
            anchors.leftMargin: 12
            anchors.right: counter.left
            anchors.rightMargin: 12
            color: Theme.promptText
            selectionColor: Theme.promptText
            selectedTextColor: Theme.promptBackground
            selectByMouse: true
            // The window is not always the one the compositor calls active, and a
            // TextInput throws its selection away the moment it is not: without this,
            // the word left over from last time is not there to be typed over.
            persistentSelection: true
            font.family: Theme.monoFamily
            font.pixelSize: Theme.bodySize

            onTextChanged: root.document.searchFor(text)

            Keys.onPressed: (event) => {
                switch (event.key) {
                // Either way of saying the word is typed: the bar goes away and
                // the cursor is left on the occurrence the search walked to.
                case Qt.Key_Escape:
                case Qt.Key_Return:
                case Qt.Key_Enter:
                    event.accepted = true
                    root.close()
                    break
                case Qt.Key_Z:
                    // Undo is the document's. Taken here so that a TextInput holding
                    // the keyboard does not answer it by unwinding the word typed
                    // into it, which is not something the writer wrote.
                    if (event.modifiers & Qt.ControlModifier) {
                        event.accepted = true
                    }
                    break
                case Qt.Key_F:
                    if (event.modifiers === Qt.ControlModifier) {
                        event.accepted = true
                        root.close()
                    }
                    break
                case Qt.Key_Up:
                case Qt.Key_Down:
                    if (event.modifiers === Qt.ControlModifier) {
                        event.accepted = true
                        root.document.cycleSearch(event.key === Qt.Key_Down ? 1 : -1)
                    }
                    break
                }
            }
        }

        // Which occurrence of how many. Asking for another one of a word that has
        // only the one moves nothing, so that answer is given here instead — and
        // given as an answer, on a ground of its own, rather than as a count.
        Rectangle {
            id: counter

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: note.width + (root.document.searchAlone ? 16 : 0)
            height: note.height + (root.document.searchAlone ? 6 : 0)
            radius: 3
            color: root.document.searchAlone ? Theme.accent : "transparent"

            Text {
                id: note

                anchors.centerIn: parent
                text: needle.text === "" ? ""
                    : root.document.searchAlone ? qsTr("only one")
                    : root.document.searchCount === 0 ? qsTr("no matches")
                    : (root.document.searchChoice + 1) + "/" + root.document.searchCount
                color: root.document.searchAlone ? Theme.promptText : Theme.muted
                font.family: Theme.monoFamily
                font.pixelSize: Theme.bodySize
            }
        }
    }
}
