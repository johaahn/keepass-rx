

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
import Lomiri.Components.Popups 1.3 as LomiriPopups
import keepassrx 1.0

import "./pages"

MainView {
    id: root
    objectName: 'mainView'
    anchorToKeyboard: true

    property var entryEditor: null
    property string draftUuid: ""

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
            uiDatabase.onDatabaseNameChanged.connect(uiDatabase.detectKeyFile);
            uiDatabase.onDatabaseTypeChanged.connect(uiDatabase.detectKeyFile);
        }
    }

    Connections {
	target: keepassrx
        function onErrorReceived(error) {
            console.error('Uncaught error (put me in a popup):', error);
        }

        function onKeyFileSet() {
            console.log('Using a key file');
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
            uiDatabase.open();
        }

        function onMasterPasswordInvalidated() {
            clearSensitiveUiState();
        }

        function onDatabaseClosed() {
            clearSensitiveUiState();
        }

        function onEntrySavedWithConflictCopy(fileName) {
            toast.show(i18n.tr("Another app changed the database while saving. Its version was kept as %1.").arg(fileName));
        }

        function onCodeLimitReached() {
            showCodeLimit();
        }

        function onStagedEditFound(title, entryChanged, error) {
            LomiriPopups.PopupUtils.open(stagedEditDialog, root, {
                entryTitle: title,
                entryChanged: entryChanged,
                error: error
            });
        }

        function onStagedDraftFound(entryUuid) {
            draftUuid = entryUuid;
            keepassrx.getSingleEntry(entryUuid);
        }

        function onSingleEntryReceived(entry) {
            if (entry && entry.uuid === draftUuid) {
                draftUuid = "";
                Qt.callLater(() => adaptiveLayout.addPageToNextColumn(
                    entriesPage,
                    Qt.resolvedUrl("pages/SingleEntry.qml"),
                    {
                        entryUuid: entry.uuid,
                        entryTitle: entry.title ? entry.title : "",
                        entryUsername: entry.username ? entry.username : "",
                        entryUrl: entry.url ? entry.url : "",
                        entryNotes: entry.notes ? entry.notes : "",
                        entryHasUsername: entry.hasUsername === true,
                        entryHasPassword: entry.hasPassword === true,
                        entryHasUrl: entry.hasUrl === true,
                        entryHasNotes: entry.hasNotes === true,
                        entryCustomFields: entry.customFields ? entry.customFields : null,
                        entryHasTotp: entry.hasTotp === true,
                        entryEntropy: entry.entropy !== undefined ? entry.entropy : null,
                        entryEntropyQuality: entry.entropyQuality ? entry.entropyQuality : "",
                        editRequested: true
                    }
                ));
            }
        }

        function onStagedEditFailed(message) {
            toast.show(i18n.tr("The unsaved edit could not be applied or discarded: %1").arg(message));
        }

        // When the UI requests getting a single value from an entry.
        function onFieldValueReceived(entryUuid, fieldName, fieldValue, fieldExtra) {
            if (fieldExtra !== "copy" || !fieldValue) {
                return;
            }

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

    function saveDraft() {
        if (entryEditor) {
            entryEditor.saveDraft();
        }
    }

    function clearSensitiveUiState() {
        clearClipboardTimer.stop();
        Clipboard.clear();
    }

    function showCodeLimit() {
        keepassrx.guiState = 'NotOpen';
        reload();
        LomiriPopups.PopupUtils.open(codeLimitDialog);
    }

    function lockUI() {
        clearSensitiveUiState();
        keepassrx.guiState = 'Locked';
        reload();
    }

    function closeUI() {
        clearSensitiveUiState();
        keepassrx.guiState = 'NotOpen';
        uiDatabase.databaseName = null;
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

    Component {
        id: codeLimitDialog

        LomiriPopups.Dialog {
            id: codeLimitDialogue
            title: i18n.tr("Passcode Disabled")
            text: i18n.tr("The passcode was entered wrong too many times. Enter the full database password to open the database.")

            Button {
                text: i18n.tr("OK")
                onClicked: LomiriPopups.PopupUtils.close(codeLimitDialogue)
            }
        }
    }

    Component {
        id: stagedEditDialog

        LomiriPopups.Dialog {
            id: stagedEditDialogue
            property string entryTitle
            property bool entryChanged: false
            property string error
            property bool busy: false

            function fieldLabel(name, custom) {
                const labels = {
                    title: i18n.tr("Title"),
                    username: i18n.tr("Username"),
                    password: i18n.tr("Password"),
                    url: i18n.tr("URL"),
                    notes: i18n.tr("Notes")
                };
                return custom ? name : labels[name];
            }

            function kindLabel(kind) {
                const labels = {
                    added: i18n.tr("Added"),
                    modified: i18n.tr("Modified"),
                    deleted: i18n.tr("Deleted")
                };
                return labels[kind];
            }

            function resolve(choice) {
                busy = true;
                keepassrx.resolveStagedEdit(choice);
            }

            Connections {
                target: keepassrx

                function onDatabaseOpened() {
                    LomiriPopups.PopupUtils.close(stagedEditDialogue);
                }

                function onStagedEditFailed(message) {
                    stagedEditDialogue.busy = false;
                }
            }

            title: error ? i18n.tr("Keep unsaved changes?") : i18n.tr("Apply unsaved changes?")
            text: error
                ? i18n.tr("Your unsaved changes could not be read: %1. Do you want to keep them and try again later, or discard them?").arg(error)
                : i18n.tr("KeePassRX closed before your changes to %1 were saved. Do you want to apply them now?").arg(entryTitle)

            Repeater {
                model: stagedEditDialogue.error ? null : keepassrx.stagedChanges

                delegate: ListItem {
                    height: changeLayout.height
                    divider.visible: false

                    ListItemLayout {
                        id: changeLayout
                        title.text: stagedEditDialogue.fieldLabel(model.name, model.custom)
                        subtitle.text: stagedEditDialogue.kindLabel(model.kind)
                    }
                }
            }

            Label {
                visible: stagedEditDialogue.entryChanged
                wrapMode: Text.Wrap
                color: theme.palette.normal.negative
                text: i18n.tr("This entry was also changed somewhere else after your edit. Applying replaces those changes, and the replaced values stay in the entry history.")
            }

            Button {
                text: stagedEditDialogue.error ? i18n.tr("Keep") : i18n.tr("Apply")
                color: theme.palette.normal.positive
                enabled: !stagedEditDialogue.busy
                onClicked: stagedEditDialogue.resolve(stagedEditDialogue.error ? "keep" : "apply")
            }

            Button {
                text: i18n.tr("Discard")
                enabled: !stagedEditDialogue.busy
                onClicked: stagedEditDialogue.resolve("discard")
            }

            ActivityIndicator {
                anchors.horizontalCenter: parent.horizontalCenter
                running: stagedEditDialogue.busy
                visible: running
            }

            Label {
                visible: keepassrx.saving
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: i18n.tr("Saving... Do not close the app.")
            }
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
