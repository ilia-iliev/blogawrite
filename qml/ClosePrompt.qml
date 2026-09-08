import QtQuick
import com.blogawrite

// The question asked on the way out of an unsaved document: a prompt line at the foot of
// the window rather than a box of buttons, answered by keystroke.
Item {
    id: root

    property Document document

    // The keyboard has to go back to the block being edited, which only the window knows.
    signal cancelled()

    anchors.fill: parent
    visible: false

    function open() {
        visible = true
        // It is answered by keystroke, so it has to hold the keyboard itself.
        forceActiveFocus()
    }

    function cancel() {
        visible = false
        root.cancelled()
    }

    function saveAndQuit() {
        // A failed save keeps the window open rather than losing the text.
        if (document.save()) {
            Qt.quit()
        }
    }

    function discardAndQuit() {
        // Nothing is written; clearing the flag just lets the close through.
        document.dirty = false
        Qt.quit()
    }

    // Nothing behind it is to be clicked while it is up. This is the modality.
    MouseArea {
        anchors.fill: parent
    }

    FootBar {
        padding: 12

        // Two centred lines: the question, then the keys that answer it.
        Column {
            width: parent.width
            spacing: 4

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: qsTr("Save changes?")
                color: Theme.promptText
                font.family: Theme.monoFamily
                font.pixelSize: Theme.bodySize
            }

            Text {
                width: parent.width
                visible: root.document.errorMessage !== ""
                text: root.document.errorMessage
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                color: Theme.promptText
                font.family: Theme.bodyFamily
                font.pixelSize: Theme.bodySize
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: qsTr("[y] [n] [esc]")
                color: Theme.promptText
                font.family: Theme.monoFamily
                font.pixelSize: Theme.bodySize
            }
        }
    }

    Keys.onPressed: (event) => {
        event.accepted = true
        switch (event.key) {
        case Qt.Key_Y:
        case Qt.Key_Return:
        case Qt.Key_Enter:
            root.saveAndQuit()
            break
        case Qt.Key_N:
            root.discardAndQuit()
            break
        case Qt.Key_Escape:
            root.cancel()
            break
        }
    }
}
