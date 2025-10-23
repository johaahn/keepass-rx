import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3 as UITK
import QtGraphicalEffects 1.0

UITK.ListItem {
    property bool passwordVisible: false
    height: units.gu(10)
    anchors.left: parent.left
    anchors.right: parent.right

    id: entireItem

    Connections {
	target: keepassrx
	onTotpReceived: (totp) => {
	    if (!totp.error) {
		UITK.Clipboard.push(totp.digits);
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
    UITK.StyleHints {
        trailingPanelColor: theme.palette.normal.foreground
        trailingForegroundColor: theme.palette.normal.foregroundText
    }

    trailingActions: UITK.ListItemActions {
        actions: [
            UITK.Action {
                visible: username
                iconName: "account"
                onTriggered: {
                    UITK.Clipboard.push(username);
                    toast.show("Username copied to clipboard (30 secs)");
		    clearClipboardTimer.start();
                }
            },
            UITK.Action {
                visible: password
                iconName: "stock_key"
                onTriggered: {
                    UITK.Clipboard.push(password)
                    toast.show("Password copied to clipboard (30 secs)")
		    clearClipboardTimer.start();
                }
            },
            UITK.Action {
                visible: url
                iconName: "external-link"
                onTriggered: {
                    if (url.indexOf('//') === -1) {
                        Qt.openUrlExternally('http://' + url)
                        return
                    }

                    Qt.openUrlExternally(url)
                }
            },
            UITK.Action {
                visible: hasTotp
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

        Image {
            id: entryImg
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
                width: parent.width
                elide: Text.ElideRight
                text: title
                font.pointSize: units.gu(1.5)
                color: theme.palette.normal.foregroundText
            }

            Text {
                width: parent.width
                elide: Text.ElideRight
                color: theme.palette.normal.backgroundTertiaryText
                text: username
            }
        }
    }

    MouseArea {
        x: parent.x
        width: entryImg.width + detailsColumn.width
        height: parent.height
        onClicked: {
            /* if (!settings.tapToReveal) { */
            /*     return */
            /* } */

            /* passwordVisible = true */
            /* timer.restart() */
	    pageStack.addPageToNextColumn(
		adaptiveLayout.primaryPage,
		Qt.resolvedUrl("../pages/SingleEntry.qml"),
		{
		    entryTitle: title,
		    entryUsername: username,
		    entryPassword: password,
		    entryUrl: url
		}
	    )
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
	    UITK.Clipboard.clear();
	    toast.show('KeepassRX: Clipboard cleared.');
	}
    }
}
