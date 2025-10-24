import QtQuick 2.12
import QtQuick.Layouts 1.12
import QtQuick.Controls 2.12
import Qt.labs.settings 1.0
import Lomiri.Components 1.3
import Lomiri.Components.Popups 1.3
import Lomiri.Content 1.3
import KeepassRx 1.0
import "../components"

Page {
    id: openDbPage

    property bool manualPath
    property bool copyingDB
    property bool pickingDB
    property bool busy
    property string errorMsg
    property double lastHeartbeat: 0

    Component.onCompleted: {
	if (keepassrx.databaseOpen) {
	    console.log('OpenDBPage: Closing an already open database. This is an anomaly.');
	    keepassrx.closeDatabase();
	}
    }

    header: PageHeader {
        id: header
        title: "KeePassRX"
        trailingActionBar.actions: [
	    Action {
		name: "Settings"
		text: i18n.tr("Settings")
		iconName: "settings"
		onTriggered: { pageStack.addPageToNextColumn(openDbPage, settingsPage) }
	    },

	    Action {
		name: "About"
		text: i18n.tr("About")
		iconName: "info"
		onTriggered: { pageStack.addPageToNextColumn(openDbPage, aboutPage) }
	    }
        ]
    }

    Settings {
        id: settings
        property string lastKey
        property string lastDB
        property int autoCloseInterval: 5
        property bool showSlowDBWarning: true
    }

    ContentPeerPicker {
        id: peerPicker
        visible: false
        showTitle: true
	//TRANSLATORS: The user is chosing a KeePass database to open.
	headerText: i18n.tr("Select Database")
	z: 10 // make sure to show above everything else.
        handler: ContentHandler.Source
        contentType: ContentType.All

	// Picker is closed by signalConnections after DB copied.
        onPeerSelected: {
            peer.selectionType = ContentTransfer.Single
            copyDatabase.target = peer.request()
        }

        onCancelPressed: peerPicker.visible = false;
    }

    function openDatabase() {
        busy = true;
        showPasswordAction.checked = false;

        if (keepassrx.isMasterPasswordEncrypted) {
	    //TRANSLATORS: Error indicating that a DB is open and locked.
	    errorMsg = i18n.tr('Cannot open another database when one is locked');
        } else {
            keepassrx.storeMasterPassword(password.text);
        }

        password.text = '';
    }

    Connections {
	target: keepassrx
	onFileSet: (path) => {
	    copyingDB = false;
	    settings.lastDB = path;
	}

	onDatabaseOpened: {
	    busy = false;
            adaptiveLayout.primaryPageSource = Qt.resolvedUrl("./EntriesPage.qml");
	}

	onDatabaseOpenFailed: (error) => {
	    busy = false;
	    errorMsg = `Error: ${error}`;
            keepassrx.invalidateMasterPassword();
	}

        onMasterPasswordStored: {
	    keepassrx.openDatabase(settings.lastDB, settings.lastKey);
        }
    }

    Connections {
        id: copyDatabase
        onStateChanged: {
            var done = (target.state === ContentTransfer.Charged)

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
	    peerPicker.visible = false;
        }
    }

    ColumnLayout {
        anchors.left: parent.left
        anchors.right: parent.right

        anchors.leftMargin: units.gu(7)
        anchors.rightMargin: units.gu(7)
        anchors.verticalCenter: parent.verticalCenter
        spacing: units.gu(2)

	RowLayout {
	    Layout.fillWidth: true

	    Rectangle {
		height: units.gu(35)
		Layout.fillWidth: true
		Layout.alignment: Qt.AlignVCenter | Qt.AlignHCenter
	        color: "transparent"

		Image {
		    id: logo
		    width: units.gu(35)
		    height: units.gu(35)
		    fillMode: Image.PreserveAspectFit
		    source: '../../assets/keepass-rx.svg'
		    x: parent.width / 2 - width / 2
		    y: parent.height / 2 - height / 2
		}
	    }
	}

        RowLayout {
            Layout.fillWidth: true

	    // The DB path when first picking a database.
            TextField {
                enabled: !busy
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

            Button {
                id: pickDB
                // TRANSLATORS: DB is the abbreviation for database
                text: i18n.tr("Pick DB")
                onClicked: {
                    pickingDB = true
                    errorMsg = ''
                    busy = false
                    peerPicker.visible = true;
                }
            }
        }

	RowLayout {
            Label {
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
            TextField {
                enabled: false
                text: settings.lastKey
                Layout.fillWidth: true
                onTextChanged: settings.lastKey = text
            }

            Button {
                visible: !settings.lastKey
		// TRANSLATORS: Pick a key file to open the password database.
                text: i18n.tr("Pick Key")
                onClicked: {
                    pickingDB = false
                    peerPicker.visible = true;
                    busy = false
                    errorMsg = ''
                }
            }
            Button {
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
	    visible: !busy

            TextField {
                id: password
                enabled: (settings.lastDB !== undefined &&
			  settings.lastDB != null &&
			  settings.lastDB.length > 0 &&
			  dbPath.text.length > 0) && !busy
                text: ''
		// TRANSLATORS: The keepass database master password
                placeholderText: i18n.tr("Password")
                echoMode: showPasswordAction.checked ? TextInput.Normal : TextInput.Password
		inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                Layout.fillWidth: true
                Keys.onReturnPressed: openDatabase()

                onTextChanged: {
                    errorMsg = ''
                }
            }

            ActionBar {
                numberOfSlots: 1
                actions: [
                    Action {
                        id: showPasswordAction
                        checkable: true
                        iconName: checked ? "view-off" : "view-on"
                    }
                ]
            }
        }

	RowLayout {
	    Layout.fillWidth: true
	    visible: !busy

            Button {
		Layout.fillWidth: true
		enabled: (
		    (dbPath.text != null && dbPath.text.length > 0) || settings.lastDB) &&
		    (settings.lastKey || password.text)
		color: LomiriColors.green
		// TRANSLATORS: Open the password database.
		text: !keepassrx.isMasterPasswordEncrypted ? i18n.tr("Open") : i18n.tr("Unlock")
		onClicked: openDatabase()
            }
	}

        ActivityIndicator {
            Layout.fillWidth: true
            running: busy
            visible: busy
        }

	Text {
	    Layout.fillWidth: true
	    Layout.preferredWidth: parent.width
	    horizontalAlignment: Qt.AlignHCenter
	    visible: busy
	    // TRANSLATORS: The database is in the process of being
	    // opened.
	    text: i18n.tr('Opening Database')
	    color: LomiriColors.slate
	}

        Label {
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
        Dialog {
            id: cpu_version_popup
	    // TRANSLATORS: Checking if the user's device has an ARMv7
	    // CPU, and if the password database is a kdbx version 3
	    // file.
            title: i18n.tr("Database version compatibility")
            modal: true
            text: i18n.tr(
                      "You are running on an ARMv7 device in which databases version 3 (kdbx3) are <b>extremely</b> slow.<br/>For your sanity, make sure your database is version 4 (kdbx4)")

            Button {
                text: "Ok"
                onClicked: {
                    PopupUtils.close(cpu_version_popup)
                }
            }
        }
    }
}
