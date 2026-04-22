import QtQuick 2.12
import Lomiri.Components 1.3
import Qt.labs.settings 1.0
import keepassrx 1.0

Page {
    id: attachmentPage
    property alias uuid: theEntry.entryUuid

    header: PageHeader {
        id: header
        title: i18n.tr("Entry Attachments")

        leadingActionBar.actions: [
            Action {
                name: "Close"
                text: i18n.tr("Close")
                iconName: "close"
                onTriggered: {
                    pageStack.removePages(attachmentPage);
                }
            }
        ]
    }

    RxUiEntry {
        id: theEntry
        app: AppState

        onReadyChanged: {
            if (theEntry.ready) {
                theEntry.loadAttachments();
            }
        }
    }
}
