import QtQuick
import com.blogawrite

// A block that is nothing but `![alt](path)`, shown at its natural size within the column.
Item {
    id: root

    property string imagePath
    property url documentBase

    signal activated()

    implicitHeight: image.height

    Image {
        id: image
        anchors.horizontalCenter: parent.horizontalCenter
        asynchronous: true
        fillMode: Image.PreserveAspectFit
        source: imagePath.includes("://") ? imagePath
              : imagePath.startsWith("/") ? "file://" + imagePath
              : documentBase + imagePath
        width: Math.min(sourceSize.width, root.width)
        height: sourceSize.width > 0 ? width * sourceSize.height / sourceSize.width : 0
    }

    Text {
        anchors.centerIn: parent
        visible: image.status === Image.Error
        text: qsTr("Missing image: %1").arg(root.imagePath)
        color: Theme.muted
        font.family: Theme.bodyFamily
        font.pixelSize: Theme.bodySize - 2
    }

    TapHandler {
        onTapped: root.activated()
    }
}
