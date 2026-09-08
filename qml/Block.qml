import QtQuick
import com.blogawrite
import com.blogawrite.text

// One row of the document. Which of the three things it is showing — an editor, a
// rendered block, a picture — is worked out here from where the cursor is standing and
// whether a selection runs through it.
Item {
    id: block

    required property int index
    required property string text
    required property string rendered
    required property string kind
    required property string imagePath

    property Document document

    // The two things a block cannot do for itself: the clipboard, which Qt hands to text
    // items alone, and the search bar, which lives at the foot of the window.
    signal copyRequested(string body)
    signal searchRequested()

    readonly property bool cursorHere: index === document.activeIndex
    // The block with the cursor is an editor, and so is the one a selection
    // running out of it is anchored in: that one has its own share of it to show.
    // Reading mode opens neither: the document is rendered the whole way down.
    readonly property bool editing: !document.reading
        && (cursorHere || index === document.selectionAnchor)
    // The far end of a selection running through this block, when there is one.
    readonly property int otherEnd: index === document.activeIndex ? document.selectionAnchor
                                                                  : document.activeIndex
    readonly property int selectionLast: Math.max(document.selectionAnchor, document.activeIndex)
    readonly property bool selected: document.selectionAnchor >= 0
        && index >= Math.min(document.selectionAnchor, document.activeIndex)
        && index <= selectionLast
    // The blocks between the two ends are covered whole. They stay rendered
    // rather than opening up: they have no partial share of the selection to show.
    readonly property bool covered: selected && index !== document.activeIndex
                                             && index !== document.selectionAnchor
    // Where this block was last clicked, handed to the editor it opens.
    property point tapPoint: Qt.point(-1, -1)

    width: ListView.view.width
    height: column.height

    // The click is spent as soon as the cursor is placed by it: a selection that
    // comes back to this block afterwards must not land on it a second time.
    onCursorHereChanged: {
        if (!cursorHere) {
            tapPoint = Qt.point(-1, -1)
        }
    }

    // Used to hand the keyboard back after the close prompt has had it.
    function refocus() {
        if (loader.item) {
            loader.item.forceActiveFocus()
        }
    }

    // A block the selection covers whole, behind it: the block itself swaps its
    // ink for the paper colour to sit on this.
    Rectangle {
        visible: block.covered
        x: column.x
        width: column.width
        height: block.height
        color: Theme.text
    }

    // The gap to the next block, filled in when the selection runs on through
    // it, so that a selection over several blocks reads as one.
    Rectangle {
        visible: block.selected && block.index < block.selectionLast
        x: column.x
        width: column.width
        y: block.height
        height: Theme.blockSpacing
        color: Theme.text
    }

    Column {
        id: column

        x: (parent.width - width) / 2
        width: Theme.columnWidth(parent.width)
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
            source: block.rendered
            selected: block.covered
            documentBase: block.document.baseUrl
            onActivated: (at) => {
                if (block.document.reading) {
                    return
                }
                block.tapPoint = at
                block.document.activate(block.index)
            }
        }
    }

    Component {
        id: imageBlock
        ImageBlock {
            imagePath: block.imagePath
            documentBase: block.document.baseUrl
            onActivated: {
                if (!block.document.reading) {
                    block.document.activate(block.index)
                }
            }
        }
    }

    Component {
        id: activeBlock
        ActiveBlock {
            id: editor

            readonly property Document doc: block.document

            source: block.text
            kind: block.kind
            current: block.cursorHere
            anchoredAt: block.index === doc.selectionAnchor ? doc.selectionPosition : -1
            beyond: doc.selectionAnchor < 0 ? ""
                  : block.otherEnd > block.index ? "below"
                  : block.otherEnd < block.index ? "above" : "here"
            initialPosition: doc.pendingCursor
            initialPoint: block.tapPoint
            lintAt: doc.lintAt
            lintLength: doc.lintLength
            lintReplacement: doc.lintReplacement
            lintWord: doc.lintWord
            searching: doc.searchActive
            searchAt: doc.searchAt
            searchSerial: doc.searchSerial
            onEdited: (body, cursor) => doc.setBlockText(block.index, body, cursor)
            onCursorMoved: (cursor) => doc.setCursorPosition(block.index, cursor)
            onUndoRequested: doc.undo()
            onSplit: (before, after) => doc.splitBlock(block.index, before, after)
            onMergeRequested: doc.mergeWithPrevious(block.index)
            onLeave: (direction) => doc.moveTo(block.index + direction, direction)
            onExtend: (direction, from) => doc.selectTo(block.index + direction, from)
            onCollapse: (at) => doc.clearSelection(at)
            onSelectAllRequested: {
                doc.selectAll()
                // The cursor lands at the foot of the document. If that is this
                // block, no new editor is made to put it there, so it sees to
                // its own share of the selection.
                if (doc.activeIndex === block.index) {
                    editor.selectAll()
                }
            }
            onTapped: doc.activate(block.index)
            onCopyRequested: (at) => block.copyRequested(doc.selectionText(at))
            onDeleteRequested: (at, insert) => doc.deleteSelection(at, insert)
            onCycleLintRequested: (direction) => doc.cycleLint(direction)
            onLearnRequested: (word) => CheckerWatch.learn(word)
            onToggleCheckingRequested: doc.toggleChecking()
            onReadingRequested: doc.toggleReading()
            onSearchRequested: block.searchRequested()
            onSettledChanged: doc.settle(editor.settled)
        }
    }
}
