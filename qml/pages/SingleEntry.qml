import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.3
import Lomiri.Components 1.3
import Qt.labs.settings 1.0

import "../components"

Page {
    property string entryTitle
    property string entryUsername
    property string entryPassword
    property string entryUrl
    property string entryNotes
    property var entryCustomFields

    header: PageHeader {
        title: entryTitle || 'Entry'
    }

    ListModel {
        id: entryModel
    }

    Component.onCompleted: {
        entryModel.append({
            fieldName: "Username",
            fieldValue: entryUsername,
            fieldShown: true
        });

        entryModel.append({
            fieldName: "Password",
            fieldValue: entryPassword,
            fieldShown: false
        });

        entryModel.append({
            fieldName: "URL",
            fieldValue: entryUrl,
            fieldShown: true
        });

        for (const [key, value] of Object.entries(entryCustomFields)) {
            entryModel.append({
                fieldName: key,
                fieldValue: value,
                fieldShown: true // TODO hide protected ones
            });
        }
    }

    Component {
        id: entryDelegate
        ListItem {
            width: parent.width
            color: "transparent"

            onClicked: {
                Clipboard.push(fieldValue);
                toast.show(`${fieldName} copied to clipboard (30 secs)`);
                clearClipboardTimer.start();
            }

            contentItem.anchors {
                bottomMargin: units.gu(0.25)
            }

            trailingActions: ListItemActions {
                actions: [
                    Action {
                        iconName: fieldShown ? "view-off" : "view-on"
                        onTriggered: {
                            fieldShown = !fieldShown;
                            const status = fieldShown ? 'Showing' : 'Hiding';
                            toast.show(`${status} field: ${fieldName}`)
                        }
                    }
                ]
            }

            ColumnLayout {
                width: parent.width
                Layout.preferredWidth: parent.width
                Layout.fillWidth: true

                SlotsLayout {
                    mainSlot: Item {
                        Column {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            y: units.gu(-2.5)

                            Row {
                                id: nameRow
                                width: parent.width
                                anchors.left: parent.left
                                anchors.right: parent.right

                                Label {
                                    text: fieldName
                                }
                            }

                            Row {
                                width: parent.width
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.leftMargin: units.gu(0.5)
                                anchors.rightMargin: units.gu(0.5)

                                Text {
                                    width: parent.width
                                    text: fieldShown ? fieldValue : "[Hidden]"
                                    color: theme.palette.normal.backgroundTertiaryText
                                }
                            }
                        }
                    }

                    Button {
                        Layout.alignment: Qt.AlignRight
                        iconName: "edit-copy"
                        SlotsLayout.position: SlotsLayout.Trailing;
                        color: "transparent"
                        width: units.gu(2)
                        onClicked: {
                            Clipboard.push(fieldValue);
                            toast.show(`${fieldName} copied to clipboard (30 secs)`);
                            clearClipboardTimer.start();
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        color: Theme.name == "Lomiri.Components.Themes.Ambiance" ? LomiriColors.porcelain : LomiriColors.inkstone
        visible: entryNotes && entryNotes.length > 0
        id: notesComponent
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: units.gu(0.25)
        anchors.rightMargin: units.gu(0.25)
        height: units.gu(13)

        RowLayout {
            id: notesLabelRow

            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: units.gu(0.25)
            anchors.rightMargin: units.gu(0.25)

            Label {
                id: notesLabel
                Layout.fillWidth: true
                Layout.fillHeight: true
                // TRANSLATORS: This is a field from the KeePass database, which holds arbitrary text.
                text: i18n.tr("Notes")
                color: LomiriColors.orange
                textSize: Label.Large
            }

            Label {
                Layout.preferredHeight: notesLabel.height
                // TRANSLATORS: Pressing this will copy the Notes field of the entry.
                text: i18n.tr("Tap to copy")
                color: LomiriColors.slate
                textSize: Label.Small
                Layout.alignment: Qt.AlignRight

                MouseArea {
                    z: 10 // to make sure anywhere in the box is copyable
                    anchors.fill: parent
                    onClicked: {
                        Clipboard.push(entryNotes);
                        toast.show(`Notes copied to clipboard (30 secs)`);
                        clearClipboardTimer.start();
                    }
                }
            }
        }

        ScrollView {
            anchors.top: notesLabelRow.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: units.gu(0.25)
            anchors.rightMargin: units.gu(0.25)
            height: units.gu(8)
            width: parent.width
            id: notesContentRow
            clip: true

            Text {
                width: notesContentRow.width
                text: entryNotes
                wrapMode: Text.WordWrap
                color: LomiriColors.ash
                verticalAlignment: Text.AlignTop
            }
        }
    }

    Row {
        id: notesDivider
        visible: notesComponent.visible
        anchors.top: notesComponent.visible ? notesComponent.bottom : header.bottom
        anchors.left: parent.left
        anchors.right: parent.right

        Rectangle {
            width: parent.width
            height: 1
            color: LomiriColors.orange
        }
    }

    Row {
        id: standardFieldsRow
        anchors.top: notesComponent.visible ? notesDivider.bottom : header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        width: parent.width
        Layout.fillWidth: true
        clip: true

        LomiriListView {
            width: parent.width
            height: parent.height

            id: lomiriListView
            model: entryModel
            delegate: entryDelegate
            highlight: Rectangle {
                color: "transparent"
            }
        }
    }

    Timer {
        id: clearClipboardTimer
        repeat: false
        running: false
        interval: 30000
        onTriggered: {
            Clipboard.clear();
            toast.show('KeePassRX: Clipboard cleared.');
        }
    }

    Popup {
        id: toast
        padding: units.dp(12)

        x: parent.width / 2 - width / 2
        y: parent.height - height - units.dp(20)

        background: Rectangle {
            color: "#111111"
            opacity: 0.7
            radius: units.dp(10)
        }

        Text {
            id: popupLabel
            anchors.fill: parent
            horizontalAlignment: Text.AlignHCenter
            color: "#ffffff"
            font.pixelSize: units.dp(14)
        }

        Timer {
            id: popupTimer
            interval: 3000
            running: true
            onTriggered: {
                toast.close()
            }
        }

        function show(text) {
            popupLabel.text = text
            open()
            popupTimer.start()
        }
    }
}
