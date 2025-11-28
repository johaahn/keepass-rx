

/*
 * Copyright (C) 2021  David Ventura
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; version 3.
 *
 * Keepass is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */
import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.3
import Qt.labs.settings 1.0
import Lomiri.Components 1.3
import keepassrx 1.0

import "./pages"

MainView {
    id: root
    objectName: 'mainView'
    anchorToKeyboard: true

    visible: true
    width: units.gu(45)
    height: units.gu(75)

    // Default values.
    RxUiDatabase {
        id: uiDatabase
        app: AppState

        Component.onCompleted: {
            uiDatabase.onDatabaseNameChanged.connect(uiDatabase.updateLastDbSet);
            uiDatabase.onDatabaseTypeChanged.connect(uiDatabase.updateLastDbSet);
        }
    }

    Connections {
	target: keepassrx
        function onErrorReceived(error) {
            console.error('Uncaught error (put me in a popup):', error);
        }

        function onDatabaseOpened() {
            keepassrx.guiState = 'Open';
	    keepassrx.encryptMasterPassword();
            adaptiveLayout.primaryPage = entriesPage;
	    entriesPage.visible = true;
        }

        function onDatabaseOpenFailed(error) {
            keepassrx.guiState = 'NotOpen';
            keepassrx.invalidateMasterPassword();
        }

        function onMasterPasswordStored() {
            if (visible) {
                uiDatabase.open();
            }
        }

        function onMasterPasswordDecrypted() {
            console.log('Re-opening database from locked state.');
            uiDatabase.open();
        }

        // When the UI requests getting a single value from one of the
        // button presses.
        function onFieldValueReceived(entryUuid, fieldName, fieldValue, fieldExtra) {
            if (fieldValue) {
                // TODO Add some better URL handling, for fields that
                // are not marked specifically with title "URL".
                if (fieldName.toLowerCase() == "url") {
                    if (fieldValue.indexOf('//') === -1) {
                        Qt.openUrlExternally('http://' + fieldValue);
                        return;
                    }

                    Qt.openUrlExternally(fieldValue);
                } else {
                    Clipboard.push(fieldValue);
                    toast.show(i18n.tr('%1 copied to clipboard (30 secs)').arg(fieldName));
                    clearClipboardTimer.start();
                }
            }
        }

        function onTotpReceived(totp) {
            if (!totp.error) {
                Clipboard.push(totp.digits);
                toast.show(
                    i18n.ctr(
                        "The totp.validFor field has an s after it. e.g. 30s (seconds)",
                        "Token '%1' copied. Valid for %2"
                    ).arg(totp.digits).arg(totp.validFor)
                );
                clearClipboardTimer.start();
            } else {
                toast.show(totp.error);
            }
        }
    }

    Component.onCompleted: {
        reload();
    }

    function lockUI() {
        keepassrx.guiState = 'Locked';
        reload();
    }

    function closeUI() {
        keepassrx.guiState = 'NotOpen';
        reload();
    }

    // Only one of dblist or unlock page can be active at a time, due
    // to conflicting signals.
    function reload() {
        if (adaptiveLayout.primaryPage) {
            adaptiveLayout.removePages(adaptiveLayout.primaryPage);
        }

        if (adaptiveLayout.primaryPageSource) {
            adaptiveLayout.removePages(adaptiveLayout.primaryPageSource);
        }

        if (keepassrx.guiState == 'Locked') {
            // User locked database
            adaptiveLayout.primaryPageSource = Qt.resolvedUrl("pages/UnlockPage.qml");
        } else if (keepassrx.guiState == 'NotOpen' && uiDatabase.databaseName) {
            // Last DB opened
            adaptiveLayout.primaryPageSource = Qt.resolvedUrl("pages/OpenDBPage.qml");
        } else {
            // No last DB
            adaptiveLayout.primaryPageSource = Qt.resolvedUrl("pages/DBList.qml");
        }
    }

    AdaptivePageLayout {
	id: adaptiveLayout
	anchors.fill: parent

        OpenDBPage {
            id: openDbPage
            visible: false
        }

	EntriesPage {
	    visible: false
	    id: entriesPage
	}

	SettingsPage {
	    visible: false
	    id: settingsPage
	}

	AboutPage {
	    visible: false
	    id: aboutPage
	}
    }

    Popup {
        id: toast
        padding: units.dp(12)

        x: parent.width / 2 - width / 2
        y: parent.height - height - units.dp(20)

        background: Rectangle {
            color: "#111111"
            opacity: 0.7
            radius: units.dp(10)
        }

        Text {
            id: popupLabel
            anchors.fill: parent
            horizontalAlignment: Text.AlignHCenter
            color: "#ffffff"
            font.pixelSize: units.dp(14)
        }

        Timer {
            id: popupTimer
            interval: 3000
            running: true
            onTriggered: {
                toast.close()
            }
        }

        function show(text) {
            popupLabel.text = text
            open()
            popupTimer.start()
        }
    }

    Timer {
        id: clearClipboardTimer
        repeat: false
        running: false
        interval: 30000
        onTriggered: {
            Clipboard.clear();
            toast.show(i18n.tr('KeePassRX: Clipboard cleared.'));
        }
    }
}
