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
        passcode.text = "";
    }

    function unlock() {
        if (!keepassrx.isMasterPasswordEncrypted) {
            errorMsg = Tr.tr("There is no locked database");
            return;
        }
        busy = true;
        errorMsg = "";
        keepassrx.decryptMasterPassword(passcode.text);
        passcode.text = "";
    }

    Component.onDestruction: clearSensitiveUiState()

    Connections {
        target: keepassrx

        onMasterPasswordDecrypted: applicationWindow.uiDatabase.open()

        // `error` is the declared signal arg name (Qt 5.6 block form).
        onDecryptionFailed: {
            unlockPage.busy = false;
            unlockPage.errorMsg = Tr.tr("Error: %1. Wrong passcode?").arg(error);
            unlockPage.clearSensitiveUiState();
        }

        // `message` is the declared signal arg name (Qt 5.6 block form).
        onDatabaseOpenFailed: {
            unlockPage.busy = false;
            unlockPage.errorMsg = Tr.tr("Error: %1").arg(message);
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
                color: Theme.secondaryColor
                text: Tr.tr("The passcode is the first five characters of the "
                    + "database password (or the whole password if less than "
                    + "five characters).")
            }

            PasswordField {
                id: passcode
                width: parent.width
                enabled: keepassrx.isMasterPasswordEncrypted && !unlockPage.busy
                label: Tr.tr("Passcode")
                placeholderText: label
                inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                EnterKey.enabled: text.length > 0
                EnterKey.iconSource: "image://theme/icon-m-enter-accept"
                EnterKey.onClicked: unlockPage.unlock()
                onTextChanged: unlockPage.errorMsg = ""
            }

            Button {
                anchors.horizontalCenter: parent.horizontalCenter
                enabled: keepassrx.isMasterPasswordEncrypted
                    && !unlockPage.busy && passcode.text.length > 0
                text: Tr.tr("Unlock")
                onClicked: unlockPage.unlock()
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
