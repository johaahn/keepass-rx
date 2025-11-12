import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3
import QtGraphicalEffects 1.0

ListItem {
    property bool passwordVisible: false
    height: units.gu(10)
    id: entireItem

    Connections {
	target: keepassrx

	onFieldValueReceived: (entryUuid, fieldName, fieldValue) => {
	    // fieldValue will be null if it's not in the entry.
	    if (fieldValue) {
		// TODO Add some better URL handling, for fields that
		// are not marked specifically as URL.
		if (fieldName.toLowerCase() == "url") {
		    if (entry.url.indexOf('//') === -1) {
			Qt.openUrlExternally('http://' + url);
			return;
		    }

		    Qt.openUrlExternally(url);
		} else {
		    Clipboard.push(fieldValue);
		    toast.show(`${fieldName} copied to clipboard (30 secs)`);
		    clearClipboardTimer.start();
		}
	    }
	}

	onSingleEntryReceived: (entry) => {
	    if (!entry) {
		console.error('no entry found?!');
	    }

	    pageStack.addPageToNextColumn(
		entriesPage,
		Qt.resolvedUrl("../pages/SingleEntry.qml"),
		{
		    entryTitle: entry.title ? entry.title : null,
		    entryUsername: entry.username ? entry.username : null,
		    entryPassword: entry.password ? entry.password : null,
		    entryUrl: entry.url ? entry.url : null,
		    entryNotes: entry.notes ? entry.notes : null,
		    entryCustomFields: entry.customFields ? entry.customFields : null
		}
	    )
	}

	onTotpReceived: (totp) => {
	    if (!totp.error) {
		Clipboard.push(totp.digits);
		toast.show("Token '" + totp.digits + "' copied. Valid for " + totp.validFor);
		clearClipboardTimer.start();
	    } else {
		toast.show(totp.error);
	    }
	}
    }

    //override the trailing action panels defaul colors. use #808080
    //for icon color, this is the default keycolor of `Icon` and will
    //then be changed to the themed color
    StyleHints {
	trailingPanelColor: theme.palette.normal.foreground
	trailingForegroundColor: theme.palette.normal.foregroundText
    }

    trailingActions: ListItemActions {
	actions: [
	    Action {
		visible: hasUsername
		iconName: "account"
		onTriggered: {
		    keepassrx.getFieldValue(uuid, "Username");
		}
	    },
	    Action {
		visible: hasPassword
		iconName: "stock_key"
		onTriggered: {
		    keepassrx.getFieldValue(uuid, "Password");
		}
	    },
	    Action {
		visible: hasURL
		iconName: "external-link"
		onTriggered: {
		    keepassrx.getFieldValue(uuid, "URL");
		}
	    },
	    Action {
		visible: hasTOTP
		iconSource: "../../assets/2fa.svg"
		iconName: "totp-code"
		onTriggered: {
		    // Need to fetch current TOTP because it shifts
		    // with time. Response is handled by the
		    // onTotpReceived event.
		    keepassrx.getTotp(uuid);
		}
	    }
	]
    }

    Rectangle {
	anchors.fill: parent
	color: theme.palette.normal.background
    }

    Row {
	anchors.leftMargin: units.gu(2)
	anchors.rightMargin: units.gu(2)
	anchors.topMargin: units.gu(1)
	anchors.bottomMargin: units.gu(1)
	anchors.fill: parent

	spacing: units.gu(1)

        Item {
            width: units.gu(5)
            height: parent.height
            visible: itemType != 'Entry'

	    Icon {
	        width: units.gu(5)
	        height: parent.height
	        y: parent.height / 2 - height / 2
	        name: itemType == 'Group' || itemType == 'Template' ? 'folder' : 'up'
	    }

	    Image {
	        id: groupEntryImg
	        fillMode: Image.PreserveAspectFit
	        source: iconPath ? iconPath : '../../assets/placeholder.png'
	        width: units.gu(2)
	        height: units.gu(2)
	        y: parent.height - height * 2
                x: parent.width - width
	    }
        }

	Image {
	    id: entryImg
	    visible: itemType == 'Entry'
	    fillMode: Image.PreserveAspectFit
	    source: iconPath ? iconPath : '../../assets/placeholder.png'
	    width: units.gu(5)
	    height: parent.height
	    y: parent.height / 2 - height / 2
	}

	Column {
	    id: detailsColumn
	    width: parent.width - parent.spacing - units.gu(6)
	    Text {
		id: titleText
		width: parent.width
		elide: Text.ElideRight
		font.pointSize: units.gu(1.5)
		color: theme.palette.normal.foregroundText
		text: title
	    }

	    Text {
		width: parent.width
		elide: Text.ElideRight
		color: theme.palette.normal.backgroundTertiaryText
		text: subtitle
	    }
	}
    }

    MouseArea {
	x: parent.x
	width: entryImg.width + detailsColumn.width
	height: parent.height
	onClicked: {
	    if (itemType == 'Group' || itemType == 'Template') {
                keepassrx.pushContainer(uuid);
	    } else if (itemType == 'Entry') {
		keepassrx.getSingleEntry(uuid);
	    } else if (itemType == 'GoBack') {
		keepassrx.popContainer();
	    }
	}
    }

    Timer {
	id: timer
	repeat: false
	interval: 1500
	onTriggered: passwordVisible = false
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
}
