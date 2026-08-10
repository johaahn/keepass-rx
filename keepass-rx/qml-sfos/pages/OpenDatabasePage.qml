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
    id: openDbPage
    allowedOrientations: defaultAllowedOrientations

    property bool busy: false
    property string errorMsg: ""

    function clearSensitiveUiState() {
        password.text = "";
    }

    function openDatabase() {
        if (!password.text) {
            return;
        }
        busy = true;
        errorMsg = "";
        keepassrx.storeMasterPassword(password.text);
        password.text = "";
    }

    Component.onDestruction: clearSensitiveUiState()

    Connections {
        target: keepassrx

        // storeMasterPassword succeeded; actually open the database.
        onMasterPasswordStored: {
            if (openDbPage.status === PageStatus.Active || openDbPage.busy) {
                applicationWindow.uiDatabase.open();
            }
        }

        // `message` is the declared signal arg name (Qt 5.6 block form).
        onDatabaseOpenFailed: {
            openDbPage.busy = false;
            //% "Error: %1"
            openDbPage.errorMsg = qsTrId("keepassrx-error").arg(message);
            openDbPage.clearSensitiveUiState();
        }
    }

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: content.height

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
                //% "Enter the database master password"
                text: qsTrId("keepassrx-enter-master-password")
            }

            PasswordField {
                id: password
                width: parent.width
                enabled: !openDbPage.busy
                //% "Master password"
                label: qsTrId("keepassrx-master-password")
                placeholderText: label
                EnterKey.enabled: text.length > 0
                EnterKey.iconSource: "image://theme/icon-m-enter-accept"
                EnterKey.onClicked: openDbPage.openDatabase()
                onTextChanged: openDbPage.errorMsg = ""
            }

            Button {
                anchors.horizontalCenter: parent.horizontalCenter
                enabled: !openDbPage.busy && password.text.length > 0
                //% "Open"
                text: qsTrId("keepassrx-open")
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
