/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation; version 3.
 */
import QtQuick 2.6
import Sailfish.Silica 1.0
import keepassrx 1.0

Page {
    id: unlockPage
    allowedOrientations: defaultAllowedOrientations

    property bool busy: false
    property string errorMsg: ""

    function clearSensitiveUiState() {
        password.text = "";
    }

    function unlock() {
        busy = true;
        errorMsg = "";
        keepassrx.storeMasterPassword(password.text);
        password.text = "";
    }

    function unlockWithSystem() {
        if (!keepassrx.isMasterPasswordEncrypted) {
            return;
        }
        busy = true;
        errorMsg = "";
        keepassrx.decryptMasterPassword("");
    }

    Component.onDestruction: clearSensitiveUiState()

    Connections {
        target: applicationWindow
        onSystemUnlockRequested: unlockPage.unlockWithSystem()
    }

    Connections {
        target: keepassrx

        onMasterPasswordDecrypted: applicationWindow.uiDatabase.open()

        onMasterPasswordStored: {
            if (unlockPage.busy) {
                applicationWindow.uiDatabase.open();
            }
        }

        onUnlockCodeRequired: {
            unlockPage.busy = false;
            unlockPage.errorMsg = Tr.tr("Authentication did not complete. Enter the password.");
        }

        onUnlockAuthRejected: {
            unlockPage.busy = false;
            unlockPage.errorMsg = Tr.tr("Authentication failed. Enter the password.");
        }

        // `error` is the declared signal arg name (Qt 5.6 block form).
        onDecryptionFailed: {
            unlockPage.busy = false;
            unlockPage.errorMsg = Tr.tr("Error: %1").arg(error);
            unlockPage.clearSensitiveUiState();
        }

        onDatabaseOpenFailed: {
            unlockPage.busy = false;
            unlockPage.errorMsg = credentialsRejected
                ? Tr.tr("Incorrect password.") : Tr.tr("Error: %1").arg(message);
            unlockPage.clearSensitiveUiState();
        }
    }

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: content.height

        PullDownMenu {
            MenuItem {
                text: Tr.tr("Close Database")
                onClicked: applicationWindow.closeDatabase()
            }
        }

        Column {
            id: content
            width: parent.width
            spacing: Theme.paddingLarge

            PageHeader {
                title: applicationWindow.uiDatabase.databaseName
                    ? applicationWindow.uiDatabase.databaseName : "KeePassRX"
            }

            Image {
                anchors.horizontalCenter: parent.horizontalCenter
                width: Math.min(parent.width / 2, Theme.itemSizeHuge * 2)
                height: width
                fillMode: Image.PreserveAspectFit
                source: "../../assets/keepass-rx.svg"
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                color: Theme.highlightColor
                text: unlockPage.busy
                    ? Tr.tr("Unlocking...")
                    : applicationWindow.lockingAvailable && keepassrx.isMasterPasswordEncrypted
                    ? Tr.tr("The database is locked. Enter the password, or tap Authenticate to unlock it with your fingerprint or security code.")
                    : Tr.tr("The database is locked. Enter the password to unlock it.")
            }

            PasswordField {
                id: password
                width: parent.width
                enabled: !unlockPage.busy
                label: Tr.tr("Password")
                placeholderText: label
                inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                EnterKey.enabled: text.length > 0
                EnterKey.iconSource: "image://theme/icon-m-enter-accept"
                EnterKey.onClicked: unlockPage.unlock()
                onTextChanged: unlockPage.errorMsg = ""
            }

            Button {
                anchors.horizontalCenter: parent.horizontalCenter
                enabled: !unlockPage.busy && password.text.length > 0
                text: Tr.tr("Unlock")
                onClicked: unlockPage.unlock()
            }

            Button {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: applicationWindow.lockingAvailable
                enabled: keepassrx.isMasterPasswordEncrypted && !unlockPage.busy
                text: Tr.tr("Authenticate")
                onClicked: unlockPage.unlockWithSystem()
            }

            BusyIndicator {
                anchors.horizontalCenter: parent.horizontalCenter
                running: unlockPage.busy
                size: BusyIndicatorSize.Medium
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                color: "red"
                visible: unlockPage.errorMsg.length > 0
                text: unlockPage.errorMsg
            }
        }
    }
}
