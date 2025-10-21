import Lomiri.Components 1.3 as UITK
import Lomiri.Components.Popups 1.3 as UC
import Lomiri.Content 1.3 as ContentHub
import QtQuick 2.12
import QtQuick.Layouts 1.12
import QtQuick.Controls 2.12
import Qt.labs.settings 1.0
import KeepassRx 1.0
import "../components"

UITK.Page {
    property bool manualPath
    property bool copyingDB
    property bool pickingDB
    property bool busy
    property string errorMsg
    property double lastHeartbeat: 0

    anchors.fill: parent

    header: UITK.PageHeader {
        id: header
        title: "KeepassRX"
        trailingActionBar.actions: [
            UITK.Action {
                iconName: "settings"
                onTriggered: {
                    stack.push(settingsPage)
                }
            }
        ]
    }

    Settings {
        id: settings
        property string lastKey
        property string lastDB
        property int autoCloseInterval: 15
        property bool showSlowDBWarning: true
    }

    ContentHub.ContentPeerPicker {
        id: peerPicker
        visible: false
        showTitle: true
        handler: ContentHub.ContentHandler.Source
        contentType: ContentHub.ContentType.All

        onPeerSelected: {
            peer.selectionType = ContentHub.ContentTransfer.Single
            signalConnections.target = peer.request()
        }
        onCancelPressed: stack.pop()
    }

    Connections {
	target: keepassrx
	onFileSet: (path) => {
	    copyingDB = false;
	    settings.lastDB = path;
	}
	onDatabaseOpened: { busy = false; }
	onDatabaseOpenFailed: (error) => {
	    busy = false;
	    errorMsg = `Error: ${error}`;
	}
    }

    Connections {
        id: signalConnections
        onStateChanged: {
            var done = (target.state === ContentHub.ContentTransfer.Charged)

            if (!done) {
                return
            }
            if (target.items.length === 0) {
                return
            }

            const filePath = String(target.items[0].url).replace('file://', '')
	    dbPath.text = filePath.split('/').pop();
	    copyingDB = true;
	    keepassrx.setFile(filePath, pickingDB);
	    stack.pop(); // Close content picker
        }
    }

    Timer {
        interval: 2000
        running: settings.autoCloseInterval > 0
        repeat: true
        onTriggered: {
            const now = new Date().getTime()
            if (lastHeartbeat === 0) {
                lastHeartbeat = now
            }

            const delta = now - lastHeartbeat
            lastHeartbeat = now
            if (stack.depth > 1
                    && delta >= settings.autoCloseInterval * 60 * 1000) {
                stack.pop(null)
            }
        }
    }

    ColumnLayout {
        anchors.left: parent.left
        anchors.right: parent.right

        anchors.leftMargin: units.gu(7)
        anchors.rightMargin: units.gu(7)
        anchors.verticalCenter: parent.verticalCenter
        spacing: units.gu(1)

	RowLayout {
	    Layout.fillWidth: true

	    Rectangle {
		height: units.gu(25)
		Layout.fillWidth: true
		Layout.alignment: Qt.AlignVCenter | Qt.AlignHCenter

		Image {
		    id: logo
		    width: units.gu(25)
		    height: units.gu(25)
		    fillMode: Image.PreserveAspectFit
		    source: '../../assets/keepass-rx.png'
		    x: parent.width / 2 - width / 2
		    y: parent.height / 2 - height / 2
		}
	    }
	}

        RowLayout {
            Layout.fillWidth: true
            UITK.TextField {
                enabled: true
		id: dbPath
                text: settings.lastDB.split('/').pop()
		Layout.fillWidth: true
		onAccepted: {
		    errorMsg = '';
		    copyingDB = true;
		    settings.lastDB = text;
		    text = settings.lastDB.split('/').pop();
		    manualPath = true;
		    keepassrx.setFile(settings.lastDB, pickingDB);
		}
            }

            UITK.Button {
                id: pickDB
                // TRANSLATORS: DB is the abbreviation for database
                text: i18n.tr("Pick DB")
                onClicked: {
                    pickingDB = true
                    errorMsg = ''
                    busy = false
                    stack.push(peerPicker)
                }
            }
        }

	RowLayout {
            UITK.Label {
		Layout.fillWidth: true
		id: manualPathLabel
		color: "gray"
		// TRANSLATORS: When the user has manually typed a file path.
		text: i18n.tr('Manual path set.')
		visible: manualPath === true
		wrapMode: Text.WordWrap
            }
	}


        RowLayout {
            UITK.TextField {
                enabled: false
                text: settings.lastKey
                Layout.fillWidth: true
                onTextChanged: settings.lastKey = text
            }

            UITK.Button {
                visible: !settings.lastKey
		// TRANSLATORS: Pick a key file to open the password database.
                text: i18n.tr("Pick Key")
                onClicked: {
                    pickingDB = false
                    stack.push(peerPicker)
                    busy = false
                    errorMsg = ''
                }
            }
            UITK.Button {
                visible: settings.lastKey
		// TRANSLATORS: Clear the selected key file.
                text: i18n.tr("Clear Key")
                onClicked: {
                    settings.lastKey = ''
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true

            UITK.TextField {
                id: password
                enabled: (settings.lastDB !== undefined &&
			  settings.lastDB != null &&
			  settings.lastDB.length > 0 &&
			  dbPath.text.length > 0)
                text: ''
		// TRANSLATORS: The keepass database master password
                placeholderText: i18n.tr("Password")
                echoMode: showPasswordAction.checked ? TextInput.Normal : TextInput.Password
                Layout.fillWidth: true
                Keys.onReturnPressed: openDatabase()

                onTextChanged: {
                    errorMsg = ''
                }
            }

            UITK.ActionBar {
                numberOfSlots: 1
                actions: [
                    UITK.Action {
                        id: showPasswordAction
                        checkable: true
                        iconSource: checked ? "../../assets/visibility_off.png" : "../../assets/visibility.png"
                    }
                ]
            }
        }

        UITK.Button {
            Layout.fillWidth: true
            enabled: (
		(dbPath.text != null && dbPath.text.length > 0) || settings.lastDB) &&
		(settings.lastKey || password.text)
            // TRANSLATORS: Open the password database
            text: i18n.tr("Open")
            onClicked: openDatabase()
        }
        UITK.ActivityIndicator {
            Layout.fillWidth: true
            running: busy
            visible: busy
        }

        UITK.Label {
            Layout.fillWidth: true
	    id: errorLabel
            text: errorMsg
	    color: "red"
	    visible: errorMsg !== undefined && errorMsg.length > 0
            wrapMode: Text.WordWrap
        }
    }

    Component {
        id: cpu_version_component
        UC.Dialog {
            id: cpu_version_popup
            title: "Database version compatibility"
            modal: true
            text: i18n.tr(
                      "You are running on an ARMv7 device in which databases version 3 (kdbx3) are <b>extremely</b> slow.<br/>For your sanity, make sure your database is version 4 (kdbx4)")

            UITK.Button {
                text: "Ok"
                onClicked: {
                    PopupUtils.close(cpu_version_popup)
                }
            }
        }
    }

    function openDatabase() {
        busy = true
	keepassrx.openDatabase(settings.lastDB, password.text, settings.lastKey);
    }
}
