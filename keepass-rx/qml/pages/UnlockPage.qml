import QtQuick 2.12
import QtQuick.Layouts 1.12
import QtQuick.Controls 2.12
import Qt.labs.settings 1.0
import Lomiri.Components 1.3
import Lomiri.Components.Popups 1.3
import Lomiri.Content 1.3
import keepassrx 1.0
import "../components"

Page {
    id: unlockPage

    property bool manualPath
    property bool copyingDB
    property bool pickingDB
    property bool busy
    property string errorMsg
    property double lastHeartbeat: 0

    Component.onCompleted: {
        if (keepassrx.databaseOpen) {
            console.log('UnlockPage: Closing an already open database. This is an anomaly.');
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
                onTriggered: { pageStack.addPageToNextColumn(unlockPage, settingsPage) }
            },

            Action {
                name: "About"
                text: i18n.tr("About")
                iconName: "info"
                onTriggered: { pageStack.addPageToNextColumn(unlockPage, aboutPage) }
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
        console.log('[Unlock] QML - Unlocking master password');
        busy = true;
        showPasswordAction.checked = false;

        if (keepassrx.isMasterPasswordEncrypted) {
            keepassrx.decryptMasterPassword(shortPassword.text);
        } else {
            errorMsg = 'There is no locked database';
        }

        shortPassword.text = '';
    }

    function resetApp() {
        console.log('Lost the master password; resetting to Open page.');
        pageStack.removePages(adaptiveLayout.primaryPage);
        adaptiveLayout.primaryPage = openDbPage;
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

        function onDecryptionFailed(error) {
            busy = false;
            errorMsg = `Error: ${error}. Wrong passcode?`;
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

        // Database name
        RowLayout {
            Layout.fillWidth: true
            width: parent.width

            Text {
                Layout.fillWidth: true
                Layout.preferredWidth: parent.width
                horizontalAlignment: Qt.AlignHCenter
                width: parent.width
                color: LomiriColors.ash
                text: uiDatabase.databaseName
            }
        }

        RowLayout {
            Label {
                Layout.fillWidth: true
                id: manualPathLabel
                color: "gray"
                // TRANSLATORS: When the user has manually typed a file path.
                text: i18n.tr('Manual path set.')
                visible: manualPath === true && !keepassrx.isMasterPasswordEncrypted
                wrapMode: Text.WordWrap
            }
        }

        RowLayout {
            Text {
                visible: keepassrx.isMasterPasswordEncrypted
                color: LomiriColors.slate
                Layout.fillWidth: true
                Layout.preferredWidth: parent.width
                horizontalAlignment: Qt.AlignHCenter
                wrapMode: Text.WordWrap
                // TRANSLATORS: Explanation of what the user must put
                // in the passcode textbox.
                text: i18n.tr(
                    'The passcode is the first five characters of the database ' +
                        'password (or the whole password if less than five characters).'
                )
            }
        }

        RowLayout {
            Layout.fillWidth: true

            TextField {
                id: shortPassword
                visible: keepassrx.isMasterPasswordEncrypted
                enabled: (keepassrx.isMasterPasswordEncrypted &&
                          uiDatabase.isLastDbSet && !busy)
                text: ''
                // TRANSLATORS: The short password for unlocking the database.
                placeholderText: i18n.tr("Passcode")
                echoMode: showPasswordAction.checked ? TextInput.Normal : TextInput.Password
                inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                Layout.fillWidth: true
                Keys.onReturnPressed: openDatabase()

                onTextChanged: {
                    errorMsg = ''
                }
            }

            ActionBar {
                visible: keepassrx.isMasterPasswordEncrypted
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
            visible: !busy && keepassrx.isMasterPasswordEncrypted
            enabled: (keepassrx.isMasterPasswordEncrypted &&
                      uiDatabase.isLastDbSet &&
                      (settings.lastKey || shortPassword.text))
            color: Theme.name == "Lomiri.Components.Themes.Ambiance" ? LomiriColors.green : LomiriColors.lightGreen
            // TRANSLATORS: Unlock a previously-opened password database.
            text: i18n.tr("Unlock")
            onClicked: openDatabase()
        }

        ActivityIndicator {
            Layout.fillWidth: true
            running: busy || (!busy && !keepassrx.isMasterPasswordEncrypted)
            visible: busy || (!busy && !keepassrx.isMasterPasswordEncrypted)
        }

        Text {
            Layout.fillWidth: true
            Layout.preferredWidth: parent.width
            horizontalAlignment: Qt.AlignHCenter
            visible: busy || (!busy && !keepassrx.isMasterPasswordEncrypted)
            // TRANSLATORS: The database is in the process of being
            // securely locked or unlocked.
            text: (busy && keepassrx.isMasterPasswordEncrypted)
                || busy // Re-opening after decrypt
                ? i18n.tr("Securely Unlocking")
                : i18n.tr("Securely Locking")
            color: LomiriColors.slate
        }

        Button {
            id: brokenButton
            visible: false
            Layout.fillWidth: true
            color: LomiriColors.red
            // TRANSLATORS: Button that appears if something broke
            // when trying to reopen the DB.
            text: i18n.tr('Database Not Opening?')
            onClicked: resetApp()
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

    // Fallback in case the database lock gets into a weird state.
    Timer {
        id: fallbackTimer
        repeat: false
        running: false
        interval: 30000
        onTriggered: {
            keepassrx.checkLockingStatus();
            brokenButton.visible = true;
        }
    }
}
