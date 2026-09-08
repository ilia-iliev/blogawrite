import QtQuick
import com.blogawrite

// A bar across the foot of the window. Four things are said down there — a file that
// would not open, what the checker makes of the cursor's surroundings, the word being
// looked for, and the question asked on the way out — and all four are said the same way:
// dark ground the width of the window, holding one column the width the document is set
// in. So all four are this, and moving the foot of the window moves all of them at once.
Rectangle {
    id: root

    // Whatever is put inside, laid out in the column and measured for the bar's height.
    default property alias content: column.children
    // Room above the content and below it.
    property real padding: 8

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: column.height + padding * 2
    color: Theme.promptBackground

    Item {
        id: column

        x: (parent.width - width) / 2
        y: root.padding
        width: Theme.columnWidth(root.width)
        height: childrenRect.height
    }
}
