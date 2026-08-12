/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation; version 3.
 *
 * KeePassRX is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * Affero General Public License for more details.
 */
import QtQuick 2.6
import Sailfish.Silica 1.0
import keepassrx 1.0

import "pages"
import "cover"

ApplicationWindow {
    id: applicationWindow

    property alias uiDatabase: uiDatabaseObj

    property string coverEntryUuid: ""
    property string coverEntryTitle: ""
    property string coverEntryIcon: ""
    property string coverGroupTitle: ""
    property bool coverHasUsername: false
    property bool coverHasPassword: false
    property bool coverHasTotp: false

    RxUiDatabase {
        id: uiDatabaseObj
        app: AppState

        Component.onCompleted: {
            uiDatabaseObj.onDatabaseNameChanged.connect(uiDatabaseObj.updateLastDbSet);
            uiDatabaseObj.onDatabaseTypeChanged.connect(uiDatabaseObj.updateLastDbSet);
            uiDatabaseObj.onDatabaseNameChanged.connect(uiDatabaseObj.detectKeyFile);
            uiDatabaseObj.onDatabaseTypeChanged.connect(uiDatabaseObj.detectKeyFile);
        }
    }

    initialPage: Component { DatabaseListPage {} }
    allowedOrientations: defaultAllowedOrientations

    // Cover can't see this window's ids; state is pushed in via the bindings below.
    cover: Component {
        CoverPage {
            guiState: keepassrx.guiState
            dbName: applicationWindow.uiDatabase.databaseName
            entryTitle: applicationWindow.coverEntryTitle
            entryIcon: applicationWindow.coverEntryIcon
            groupTitle: applicationWindow.coverGroupTitle
            hasPassword: applicationWindow.coverHasPassword
            hasTotp: applicationWindow.coverHasTotp
            hasUsername: applicationWindow.coverHasUsername
            onRequestUnlock: applicationWindow.activate()
            onRequestCopy: {
                var uuid = applicationWindow.coverEntryUuid;
                if (kind === "password") {
                    keepassrx.getFieldValue(uuid, "Password");
                } else if (kind === "totp") {
                    keepassrx.getTotp(uuid);
                } else if (kind === "username") {
                    keepassrx.getFieldValue(uuid, "Username");
                }
            }
        }
    }

    function notify(message) {
        bannerLabel.text = message;
        banner.opacity = 1;
        bannerTimer.restart();
    }

    Rectangle {
        id: banner
        z: 10000
        opacity: 0
        Behavior on opacity { FadeAnimation {} }
        color: Theme.highlightDimmerColor
        radius: Theme.paddingSmall
        width: Math.min(bannerLabel.implicitWidth + 2 * Theme.paddingLarge,
                        applicationWindow.width - 2 * Theme.paddingLarge)
        height: bannerLabel.implicitHeight + 2 * Theme.paddingMedium
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.paddingLarge

        Label {
            id: bannerLabel
            anchors.centerIn: parent
            width: applicationWindow.width - 4 * Theme.paddingLarge
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            color: Theme.primaryColor
            font.pixelSize: Theme.fontSizeSmall
        }

        Timer {
            id: bannerTimer
            interval: 3000
            onTriggered: banner.opacity = 0
        }
    }

    function clearSensitiveUiState() {
        clearClipboardTimer.stop();
        Clipboard.text = "";
        coverEntryUuid = "";
        coverEntryTitle = "";
        coverEntryIcon = "";
        coverGroupTitle = "";
        coverHasUsername = false;
        coverHasPassword = false;
        coverHasTotp = false;
    }

    Timer {
        id: clearClipboardTimer
        repeat: false
        interval: 30000
        onTriggered: {
            Clipboard.text = "";
            applicationWindow.notify(Tr.tr("Clipboard cleared."));
        }
    }

    function closeDatabase() {
        clearSensitiveUiState();
        keepassrx.invalidateMasterPassword();
        keepassrx.guiState = 'NotOpen';
        pageStack.replaceAbove(null, Qt.resolvedUrl("pages/DatabaseListPage.qml"));
    }

    function lockDatabase() {
        clearSensitiveUiState();
        keepassrx.closeDatabase();
        keepassrx.guiState = 'Locked';
        pageStack.replaceAbove(null, Qt.resolvedUrl("pages/UnlockPage.qml"));
    }

    // Lock (or close) the database after a spell minimized to the cover.
    onApplicationActiveChanged: {
        if (applicationActive) {
            idleLockTimer.stop();
        } else if (SettingsBridge.lockWhenIdle && keepassrx.guiState === 'Open') {
            idleLockTimer.restart();
        }
    }

    Timer {
        id: idleLockTimer
        interval: 30000
        repeat: false
        onTriggered: {
            if (keepassrx.guiState !== 'Open') {
                return;
            }
            if (SettingsBridge.databaseLocking) {
                applicationWindow.lockDatabase();
            } else {
                applicationWindow.closeDatabase();
            }
        }
    }

    Connections {
        target: keepassrx

        onErrorReceived: {
            console.error("keepassrx error:", error);
            applicationWindow.notify(error);
        }

        onDatabaseOpened: {
            keepassrx.guiState = 'Open';
            keepassrx.encryptMasterPassword();
            pageStack.replaceAbove(null, Qt.resolvedUrl("pages/EntriesPage.qml"));
        }

        onViewModeChanged: {
            if (keepassrx.databaseOpen) {
                pageStack.replaceAbove(null, Qt.resolvedUrl("pages/EntriesPage.qml"));
            }
        }

        onDatabaseOpenFailed: {
            keepassrx.guiState = 'NotOpen';
            keepassrx.invalidateMasterPassword();
        }

        onDatabaseClosed: applicationWindow.clearSensitiveUiState()
        onMasterPasswordInvalidated: applicationWindow.clearSensitiveUiState()

        // Qt 5.6 Connections expose signal args by their declared names.
        onFieldValueReceived: {
            if (field_extra !== "copy" || !field_value) {
                return;
            }

            if (field_name.toLowerCase() === "url") {
                var url = field_value.indexOf('//') === -1
                    ? 'http://' + field_value
                    : field_value;
                Qt.openUrlExternally(url);
            } else {
                Clipboard.text = field_value;
                applicationWindow.notify(Tr.tr("%1 copied to clipboard (30 secs)").arg(field_name));
                clearClipboardTimer.restart();
            }
        }

        onTotpReceived: {
            if (!totp.error) {
                Clipboard.text = totp.digits;
                applicationWindow.notify(Tr.tr("Token '%1' copied. Valid for %2").arg(totp.digits).arg(totp.validFor));
                clearClipboardTimer.restart();
            } else {
                applicationWindow.notify(totp.error);
            }
        }
    }
}
