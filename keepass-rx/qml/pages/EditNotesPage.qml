import QtQuick 2.12
import Lomiri.Components 1.3

Page {
    id: editNotesPage
    property var editor

    header: PageHeader {
        id: header
        title: i18n.tr("Notes")

        leadingActionBar.actions: [
            Action {
                name: "Back"
                text: i18n.tr("Back")
                iconName: "previous"
                onTriggered: pageStack.removePages(editNotesPage)
            }
        ]
    }

    TextArea {
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: units.gu(1)
        text: editor.notesText
        wrapMode: Text.Wrap
        focus: true
        onTextChanged: editor.notesText = text
    }
}
