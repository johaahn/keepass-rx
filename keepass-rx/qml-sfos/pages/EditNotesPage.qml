import QtQuick 2.6
import Sailfish.Silica 1.0
import keepassrx 1.0

Page {
    id: notesPage
    allowedOrientations: defaultAllowedOrientations

    property var editor

    SilicaFlickable {
        id: flickable
        anchors.fill: parent
        contentHeight: column.height

        Column {
            id: column
            width: flickable.width

            PageHeader {
                title: Tr.tr("Notes")
            }

            TextArea {
                width: parent.width
                placeholderText: Tr.tr("Notes")
                labelVisible: false
                text: notesPage.editor.notesText
                focus: true
                Component.onCompleted: cursorPosition = text.length
                onTextChanged: notesPage.editor.notesText = text
            }
        }

        VerticalScrollDecorator {}
    }
}
