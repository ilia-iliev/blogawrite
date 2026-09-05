import QtQuick
import com.blogawrite
import com.blogawrite.text

// A block shown as rendered markdown: Qt hides the markers and applies typography.
Item {
    id: root

    property string source
    property url documentBase
    // Covered whole by a selection: shown the way an editor shows one, ink for paper.
    property bool selected: false

    // Carries where it was clicked, so the editor can put the cursor under the pointer.
    signal activated(point at)

    implicitHeight: label.contentHeight

    // A read-only TextEdit rather than a Text, which is the lighter of the two and was
    // what this used to be: only a TextEdit hands out the document its markdown was
    // parsed into, and only a QSyntaxHighlighter over that document can mark a word.
    // Line spacing goes with it — a TextEdit has no property for it, so the highlighter
    // sets it on the document's blocks.
    TextEdit {
        id: label

        width: root.width
        text: root.source
        textFormat: TextEdit.MarkdownText
        baseUrl: root.documentBase
        wrapMode: TextEdit.WordWrap
        readOnly: true
        // The tap below opens a real editor in this block's place; this one is only ever
        // looked at, so it takes neither the keyboard nor a cursor of its own.
        activeFocusOnPress: false
        selectByMouse: false
        cursorVisible: false
        color: root.selected ? Theme.background : Theme.text
        font.family: Theme.bodyFamily
        font.pixelSize: Theme.bodySize
        onLinkActivated: (link) => Qt.openUrlExternally(link)

        RenderedHighlighter {
            target: label.textDocument
            // A block covered whole by a selection reads as one colour, so the wash
            // goes with everything else that would break it up.
            lint: root.selected ? "transparent" : Theme.lint
            lineHeight: Theme.lineHeight
            monoFamily: Theme.monoFamily
            codeSize: Theme.codeSize
        }
    }

    TapHandler {
        onTapped: (eventPoint) => root.activated(eventPoint.position)
    }
}
