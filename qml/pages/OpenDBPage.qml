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
    property string databaseName
    property bool busy
    property string errorMsg
    property double lastHeartbeat: 0

    Component.onCompleted: {
        if (databaseName) {
            keepassrx.lastDB = databaseName;
        } else {
            if (keepassrx.lastDB) {
                databaseName = keepassrx.lastDB;
            }
        }
    }

    header: PageHeader {
        id: header
        title: databaseName || keepassrx.lastDB
        leadingActionBar.actions: [
            Action {
                name: "Back"
                text: i18n.tr("Back")
                iconName: "previous"
                onTriggered: {
                    // When going back, remove the setting.
                    console.log('going back');
                    keepassrx.lastDB = null;
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
        property string lastDB
        property int autoCloseInterval: 5
        property bool showSlowDBWarning: true
        property bool easyOpen: true
    }

    function openDatabase() {
        console.log('[OpenDB] QML - Storing password');
        busy = true;
        showPasswordAction.checked = false;

        keepassrx.lastDB = databaseName;

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

        onDatabaseOpened: {
            busy = false;
        }

        onDatabaseOpenFailed: (error) => {
            busy = false;
            errorMsg = `Error: ${error}`;
        }

        onLockingStatusReceived: (status) => {
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
                color: LomiriColors.ash
                width: parent.width
                text: databaseName || keepassrx.lastDB
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

            ActionBar {
                visible: !busy
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

        Button {
            Layout.fillWidth: true
            visible: !busy
            enabled: !busy && password.text
            color: Theme.name == "Lomiri.Components.Themes.Ambiance" ? LomiriColors.green : LomiriColors.lightGreen
            // TRANSLATORS: Open the database after password entered.
            text: i18n.tr("Open")
            onClicked: openDatabase()
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
