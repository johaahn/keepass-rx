import QtQuick 2.12
import Lomiri.Components 1.3
import Lomiri.Content 1.3
import Qt.labs.settings 1.0
import keepassrx 1.0

Page {
    id: attachmentPage
    property alias uuid: theEntry.entryUuid
    property var activeTransfer: null
    property string exportPath: ""
    property string exportUrl: ""
    property string exportFileName: ""

    function cleanupExportFile() {
        if (exportPath !== "") {
            theEntry.cleanupExportedAttachment(exportPath);
        }
    }

    function clearExportState(cleanup) {
        if (cleanup) {
            cleanupExportFile();
        }
        exportPath = "";
        exportUrl = "";
        exportFileName = "";
        activeTransfer = null;
        exportPeerPicker.visible = false;
        exportTransferConnection.target = null;
    }

    function beginAttachmentExport(attachmentName) {
        clearExportState(true);

        const result = theEntry.exportAttachment(attachmentName);
        if (!result.ok) {
            toast.show(result.error || i18n.tr("Unable to export attachment."));
            return;
        }

        exportPath = result.path;
        exportUrl = result.url;
        exportFileName = result.fileName;
        exportPeerPicker.visible = true;
    }

    function viewOrExportAttachment(attachmentName) {
        const result = theEntry.viewAttachment(attachmentName);

        if (result.ok && result.canView && result.viewType === "text") {
            pageStack.addPageToNextColumn(
                attachmentPage,
                Qt.resolvedUrl("ViewAttachment.qml"),
                {
                    attachmentName: attachmentName,
                    displayName: result.fileName || attachmentName,
                    mimeType: result.mimeType || "",
                    text: result.text || "",
                    sourcePage: attachmentPage
                }
            );
            return;
        }

        beginAttachmentExport(attachmentName);
    }

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

    ContentPeerPicker {
        id: exportPeerPicker
        visible: false
        showTitle: true
        headerText: i18n.tr("Export Attachment")
        z: 10
        handler: ContentHandler.Destination
        contentType: ContentType.All

        onPeerSelected: {
            peer.selectionType = ContentTransfer.Single;
            activeTransfer = peer.request();
            exportTransferConnection.target = activeTransfer;
        }

        onCancelPressed: {
            clearExportState(true);
        }
    }

    ContentTransferHint {
        anchors.fill: parent
        activeTransfer: attachmentPage.activeTransfer
    }

    Component {
        id: exportContentItem
        ContentItem {}
    }

    Connections {
        id: exportTransferConnection
        target: null

        function onStateChanged() {
            if (!activeTransfer) {
                return;
            }

            if (activeTransfer.state === ContentTransfer.InProgress) {
                activeTransfer.items = [
                    exportContentItem.createObject(attachmentPage, {
                        "url": exportUrl
                    })
                ];
                activeTransfer.state = ContentTransfer.Charged;
                exportPeerPicker.visible = false;
                return;
            }

            if (activeTransfer.state === ContentTransfer.Charged) {
                exportPeerPicker.visible = false;
                toast.show(i18n.tr("%1 ready to export.").arg(exportFileName));
                return;
            }

            if (activeTransfer.state === ContentTransfer.Collected) {
                const fileName = exportFileName;
                cleanupExportFile();
                clearExportState(false);
                toast.show(i18n.tr("%1 exported.").arg(fileName));
                return;
            }

            if (activeTransfer.state === ContentTransfer.Aborted ||
                    activeTransfer.state === ContentTransfer.Finalized) {
                clearExportState(true);
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
                viewOrExportAttachment(attachmentName);
            }
        }
    }

    Component.onDestruction: {
        clearExportState(true);
    }
}
