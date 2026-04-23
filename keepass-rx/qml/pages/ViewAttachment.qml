import QtQuick 2.12
import Lomiri.Components 1.3
import Qt.labs.settings 1.0
import keepassrx 1.0

Page {
    id: viewAttachmentPage
    property string attachmentName
    property string displayName
    property string mimeType
    property string text
    property var sourcePage

    header: PageHeader {
        id: header
        title: displayName || attachmentName || i18n.tr("Attachment")

        leadingActionBar.actions: [
            Action {
                name: "Close"
                text: i18n.tr("Close")
                iconName: "close"
                onTriggered: {
                    pageStack.removePages(viewAttachmentPage);
                }
            }
        ]

        trailingActionBar.actions: [
            Action {
                name: "Export"
                text: i18n.tr("Export")
                iconName: "document-save-as"
                onTriggered: {
                    if (sourcePage) {
                        sourcePage.beginAttachmentExport(attachmentName);
                    }
                    pageStack.removePages(viewAttachmentPage);
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
        text: viewAttachmentPage.text
        readOnly: true
        cursorVisible: selectedText !== ""
        selectByMouse: true
        persistentSelection: true
        wrapMode: Text.Wrap
    }
}
