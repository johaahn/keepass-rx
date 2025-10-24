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
    }

    Component {
	id: entryDelegate
	ListItem {
	    width: parent.width
	    color: "transparent"

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

    Item {
	anchors.top: header.bottom
	anchors.left: parent.left
	anchors.right: parent.right
	anchors.bottom: parent.bottom

	LomiriListView {
            id: lomiriListView
            anchors.fill: parent
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
