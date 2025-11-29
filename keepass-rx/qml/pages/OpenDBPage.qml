import QtQuick 2.12
import QtQuick.Layouts 1.12
import QtQuick.Controls 2.12
import Qt.labs.settings 1.0
import Lomiri.Components 1.3
import Lomiri.Components.Popups 1.3
import Lomiri.Content 1.3
import "../components"

Page {
    id: openDbPage
    property bool busy
    property string errorMsg
    property double lastHeartbeat: 0

    function keyFileColor() {
        return uiDatabase.isKeyFileSet
            ? LomiriColors.orange
            : theme.palette.normal.backgroundSecondaryText
    }

    function keyFileText() {
        if (uiDatabase.isKeyFileSet) {
            if (uiDatabase.isKeyFileDetected) {
                return i18n.tr("Auto-detected key file for database.")
            } else {
                return i18n.tr("A key file will be used to open this database. Tap lock to clear.")
            }
        } else {
            return i18n.tr("Tap the key icon to use a key file.")
        }
    }

    ContentPeerPicker {
        id: keyFilePicker
        visible: false
        showTitle: true
        //TRANSLATORS: The user is selecting a key file to open database.
        headerText: i18n.tr("Select Key File")
        z: 10 // make sure to show above everything else.
        handler: ContentHandler.Source
        contentType: ContentType.All

        // Picker is closed by signalConnections after key file chosen.
        onPeerSelected: {
            peer.selectionType = ContentTransfer.Single;
            storeKeyFileConnection.target = peer.request();
        }

        onCancelPressed: keyFilePicker.visible = false;
    }

    Connections {
        id: storeKeyFileConnection

        function onStateChanged() {
            var done = target.state === ContentTransfer.Charged;

            if (!done) {
                return;
            }

            if (target.items.length === 0) {
                return;
            }

            const filePath = String(target.items[0].url).replace('file://', '');
            uiDatabase.useKeyFile(filePath);
            target.finalize();
            keyFilePicker.visible = false;
        }
    }


    header: PageHeader {
        id: header
        title: uiDatabase.databaseName
        leadingActionBar.actions: [
            Action {
                name: "Back"
                text: i18n.tr("Back")
                iconName: "previous"
                onTriggered: {
                    // When going back, remove last DB.
                    uiDatabase.databaseName = null;
                    root.reload();
                }
            }
        ]

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
        property int autoCloseInterval: 5
        property bool showSlowDBWarning: true
        property bool easyOpen: true
    }

    function openDatabase() {
        console.log('[OpenDB] QML - Storing password');
        busy = true;
        showPasswordAction.checked = false;

        if (keepassrx.isMasterPasswordEncrypted) {
            // TODO should not be able to be in this state
            console.error('Why are we in this state?');
        } else {
            keepassrx.storeMasterPassword(password.text);
        }

        password.text = '';
    }

    Connections {
        target: keepassrx

        function onDatabaseOpened() {
            busy = false;
        }

        function onDatabaseOpenFailed(error) {
            busy = false;
            errorMsg = `Error: ${error}`;
        }

        function onLockingStatusReceived(status) {
            if (status === 'unset') {
                resetApp();
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
                color: "transparent"

                Image {
                    id: logo
                    width: units.gu(25)
                    height: units.gu(25)
                    fillMode: Image.PreserveAspectFit
                    source: '../../assets/keepass-rx.svg'
                    x: parent.width / 2 - width / 2
                    y: parent.height / 2 - height / 2
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            width: parent.width

            Text {
                Layout.fillWidth: true
                Layout.preferredWidth: parent.width
                horizontalAlignment: Qt.AlignHCenter
                color: theme.palette.normal.backgroundSecondaryText
                width: parent.width
                text: uiDatabase.databaseName
            }
        }

        RowLayout {
            Layout.fillWidth: true
            width: parent.width

            Text {
                Layout.fillWidth: true
                Layout.preferredWidth: parent.width
                horizontalAlignment: Qt.AlignHCenter
                color: theme.palette.normal.backgroundTertiaryText
                width: parent.width
                text: uiDatabase.databaseTypeTranslated
            }
        }

        RowLayout {
            Text {
                visible: !busy
                color: LomiriColors.slate
                Layout.fillWidth: true
                Layout.preferredWidth: parent.width
                horizontalAlignment: Qt.AlignHCenter
                wrapMode: Text.WordWrap
                // TRANSLATORS: The user must type the database master password.
                text: i18n.tr('Enter the database master password')
            }
        }

        RowLayout {
            Layout.fillWidth: true

            TextField {
                id: password
                visible: !busy
                enabled: !busy
                text: ''
                // TRANSLATORS: The master password for opening the database.
                placeholderText: i18n.tr("Master Password")
                echoMode: showPasswordAction.checked ? TextInput.Normal : TextInput.Password
                inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                Layout.fillWidth: true
                Keys.onReturnPressed: openDatabase()

                onTextChanged: {
                    errorMsg = ''
                }
            }
        }

        RowLayout {
            Button {
                id: openDatabaseButton
                Layout.fillWidth: true
                visible: !busy
                enabled: !busy && password.text
                color: Theme.name == "Lomiri.Components.Themes.Ambiance" ? LomiriColors.green : LomiriColors.lightGreen
                // TRANSLATORS: Open the database after password entered.
                text: i18n.tr("Open")
                onClicked: openDatabase()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter | Qt.AlignHCenter

            ActionBar {
                id: actionBar
                visible: !busy
                numberOfSlots: 2

                actions: [
                    Action {
                        id: keyFileAction
                        iconName: uiDatabase.isKeyFileSet ? "lock" : "stock_key"
                        enabled: !uiDatabase.isKeyFileDetected
                        text: i18n.tr('Key File')
                        description: i18n.tr('Provide a key file for opening the database.')
                        onTriggered: {
                            if (uiDatabase.isKeyFileSet) {
                                uiDatabase.clearKeyFile();
                            }
                            else {
                                keyFilePicker.visible = true;
                            }
                        }
                    },
                    Action {
                        id: showPasswordAction
                        checkable: true
                        iconName: checked ? "view-off" : "view-on"
                        text: i18n.tr('Show Password')
                        description: i18n.tr('Toggle password field visibility')
                    }
                ]
            }
        }

        RowLayout {
            visible: !busy
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter | Qt.AlignHCenter
            spacing: units.gu(1)

            Label {
                wrapMode: Text.WordWrap
                color: keyFileColor()
                text: keyFileText()
            }
        }

        RowLayout {
            visible: !busy
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter | Qt.AlignHCenter
            spacing: units.gu(1)

            Label {
                wrapMode: Text.WordWrap
                color: theme.palette.normal.backgroundSecondaryText
                text: showPasswordAction.checked
                    ? i18n.tr("Tap the eye icon to hide the password.")
                    : i18n.tr("Tap the eye icon to show the password.")
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
            text: i18n.tr("Opening")
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
}
