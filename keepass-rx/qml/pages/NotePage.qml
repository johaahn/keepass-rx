import QtQuick 2.12
import Lomiri.Components 1.3
import Qt.labs.settings 1.0
import keepassrx 1.0

Page {
    id: notePage
    property string note

    header: PageHeader {
        id: header
        title: i18n.tr("Entry Notes")

        leadingActionBar.actions: [
            Action {
                name: "Close"
                text: i18n.tr("Close")
                iconName: "close"
                onTriggered: {
                    pageStack.removePages(notePage);
                }
            }
        ]

        trailingActionBar.actions: [
            Action {
                name: "Open Source Licenses"
                iconName: "edit-copy"
                onTriggered: {
                    Clipboard.push(note);
                    toast.show(i18n.tr('Note copied to clipboard'));
                }
            }
        ]
    }

    TextArea {
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: units.gu(1.0)
        anchors.rightMargin: units.gu(1.0)
        text: note
        readOnly: true
        cursorVisible: selectedText !== ""
        selectByMouse: true
        persistentSelection: true
        wrapMode: Text.Wrap
    }
}
