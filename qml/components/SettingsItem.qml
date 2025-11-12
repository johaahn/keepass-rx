import QtQuick 2.0
import Lomiri.Components 1.3 as UITK

Row {
    property string title
    property string description
    property alias control: loader.sourceComponent

    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: units.gu(2)
    anchors.rightMargin: units.gu(2)
    spacing: units.gu(2)

    Column {
        spacing: units.gu(0.2)
        width: parent.width - loader.width - parent.spacing
        anchors.verticalCenter: parent.verticalCenter
        UITK.Label {
            anchors.left: parent.left
            anchors.right: parent.right
            text: title
            wrapMode: Text.WordWrap
        }
        UITK.Label {
            visible: description !== ''
            anchors.left: parent.left
            anchors.right: parent.right
            text: description
            color: theme.palette.normal.raisedText
            wrapMode: Text.WordWrap
            font.pixelSize: units.gu(1.6)
        }
    }
    Loader {
        anchors.verticalCenter: parent.verticalCenter
        width: units.gu(6)
        id: loader
    }
}
