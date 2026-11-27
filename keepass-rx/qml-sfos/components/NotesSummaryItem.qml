import QtQuick 2.6
import Sailfish.Silica 1.0

BackgroundItem {
    id: notesItem
    property string notesText
    property bool modified
    height: notesColumn.height + 2 * Theme.paddingMedium

    Icon {
        id: notesChevron
        anchors {
            right: parent.right
            rightMargin: Theme.horizontalPageMargin
            verticalCenter: parent.verticalCenter
        }
        source: "image://theme/icon-m-right"
        highlighted: notesItem.highlighted
    }

    Column {
        id: notesColumn
        x: Theme.horizontalPageMargin
        y: Theme.paddingMedium
        width: notesChevron.x - Theme.paddingMedium - x

        Label {
            width: parent.width
            text: notesItem.modified ? Tr.tr("Notes (modified)") : Tr.tr("Notes")
            font.pixelSize: Theme.fontSizeSmall
            color: notesItem.highlighted || notesItem.modified
                ? Theme.secondaryHighlightColor
                : Theme.secondaryColor
        }

        Label {
            width: parent.width
            text: notesItem.notesText.length > 0 ? notesItem.notesText : Tr.tr("No notes")
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
            color: notesItem.highlighted ? Theme.highlightColor : Theme.primaryColor
        }
    }
}
