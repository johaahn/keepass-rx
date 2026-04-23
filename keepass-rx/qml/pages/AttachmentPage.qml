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

    LomiriListView {
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        width: parent.width
        height: parent.height - header.height

        id: attachmentList
        model: theEntry.attachments

        delegate: ListItem {
            height: layout.height + (divider.visible ? divider.height : 0)

            // From RxUiAttachment
            ListItemLayout {
                id: layout
                title.text: attachmentName
                subtitle.text: `${attachmentSize} bytes`

                Icon {
                    name: "document-save-as"
                    SlotsLayout.overrideVerticalPositioning: true
                    SlotsLayout.position: SlotsLayout.Trailing
                    width: units.gu(3)
                    height: units.gu(3)
                    y: layout.subtitle.y - baselineOffset
                }
            }

            onClicked: {
                console.log('want to open attachment', attachmentName);
            }
        }
    }
}
