import QtQuick
import com.blogawrite

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

    Text {
        id: label

        width: root.width
        text: root.source
        textFormat: Text.MarkdownText
        baseUrl: root.documentBase
        wrapMode: Text.WordWrap
        color: root.selected ? Theme.background : Theme.text
        font.family: Theme.bodyFamily
        font.pixelSize: Theme.bodySize
        lineHeight: Theme.lineHeight
        lineHeightMode: Text.ProportionalHeight
        onLinkActivated: (link) => Qt.openUrlExternally(link)
    }

    TapHandler {
        onTapped: (eventPoint) => root.activated(eventPoint.position)
    }
}
