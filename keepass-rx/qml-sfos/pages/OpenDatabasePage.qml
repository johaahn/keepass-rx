/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation; version 3.
 */
import QtQuick 2.6
import Sailfish.Silica 1.0
import Sailfish.Pickers 1.0
import keepassrx 1.0

Page {
    id: openDbPage
    allowedOrientations: defaultAllowedOrientations

    property bool busy: false
    property string errorMsg: ""
    property bool credentialsRejected: false

    function clearSensitiveUiState() {
        password.text = "";
    }

    function openDatabase() {
        if (!password.text) {
            return;
        }
        busy = true;
        errorMsg = "";
        credentialsRejected = false;
        keepassrx.storeMasterPassword(password.text);
        password.text = "";
    }

    function keyFileDescription() {
        if (!applicationWindow.uiDatabase.isKeyFileSet) {
            return "";
        }
        return applicationWindow.uiDatabase.isKeyFileDetected
            ? Tr.tr("Automatically detected a key file for this database.")
            : Tr.tr("A key file will be used to open this database.");
    }

    function credentialsMessage() {
        return applicationWindow.uiDatabase.isKeyFileSet
            ? Tr.tr("Incorrect password or key file.")
            : Tr.tr("Incorrect password, or this database needs a key file. Pull down to add one.");
    }

    Component.onDestruction: clearSensitiveUiState()

    Component {
        id: keyFilePicker
        FilePickerPage {
            title: Tr.tr("Select Key File")
            onSelectedContentPropertiesChanged: {
                openDbPage.credentialsRejected = false;
                applicationWindow.uiDatabase.useKeyFile(selectedContentProperties.filePath);
            }
        }
    }

    Connections {
        target: keepassrx

        // storeMasterPassword succeeded; actually open the database.
        onMasterPasswordStored: {
            if (openDbPage.status === PageStatus.Active || openDbPage.busy) {
                applicationWindow.uiDatabase.open();
            }
        }

        // `message` and `credentialsRejected` are the declared signal arg names (Qt 5.6 block form).
        onDatabaseOpenFailed: {
            openDbPage.busy = false;
            openDbPage.credentialsRejected = credentialsRejected;
            openDbPage.errorMsg = credentialsRejected
                ? "" : Tr.tr("Error: %1").arg(message);
            openDbPage.clearSensitiveUiState();
        }
    }

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: content.height

        PullDownMenu {
            visible: !applicationWindow.uiDatabase.isKeyFileDetected

            MenuItem {
                text: applicationWindow.uiDatabase.isKeyFileSet
                    ? Tr.tr("Remove Key File") : Tr.tr("Use Key File")
                onClicked: {
                    openDbPage.credentialsRejected = false;
                    if (applicationWindow.uiDatabase.isKeyFileSet) {
                        applicationWindow.uiDatabase.clearKeyFile();
                    } else {
                        pageStack.push(keyFilePicker);
                    }
                }
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
                text: Tr.tr("Enter the database master password")
            }

            PasswordField {
                id: password
                width: parent.width
                enabled: !openDbPage.busy
                label: Tr.tr("Master password")
                placeholderText: label
                EnterKey.enabled: text.length > 0
                EnterKey.iconSource: "image://theme/icon-m-enter-accept"
                EnterKey.onClicked: openDbPage.openDatabase()
                errorHighlight: openDbPage.credentialsRejected
                description: openDbPage.credentialsRejected
                    ? openDbPage.credentialsMessage()
                    : openDbPage.keyFileDescription()
                onTextChanged: {
                    if (text.length > 0) {
                        openDbPage.errorMsg = "";
                        openDbPage.credentialsRejected = false;
                    }
                }
            }

            Button {
                anchors.horizontalCenter: parent.horizontalCenter
                enabled: !openDbPage.busy && password.text.length > 0
                text: Tr.tr("Open")
                onClicked: openDbPage.openDatabase()
            }

            BusyIndicator {
                anchors.horizontalCenter: parent.horizontalCenter
                running: openDbPage.busy
                size: BusyIndicatorSize.Medium
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                color: "red"
                visible: openDbPage.errorMsg.length > 0
                text: openDbPage.errorMsg
            }
        }
    }
}
