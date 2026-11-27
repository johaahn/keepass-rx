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

import "../components"

Page {
    id: entryPage
    allowedOrientations: defaultAllowedOrientations

    property string entryUuid
    property string entryTitle
    property string entryUsername: ""
    property string entryUrl: ""
    property string entryNotes: ""
    property bool entryHasUsername: false
    property bool entryHasPassword: false
    property var entryEntropy: null
    property bool entryHasUrl: false
    property bool entryHasNotes: false
    property bool entryHasTotp: false
    property var entryCustomFields: null
    property string entryIconUrl: ""
    property bool openEditor: false

    function hasEntropy() {
        return entryHasPassword && entryEntropy !== undefined
            && entryEntropy !== null && !isNaN(Number(entryEntropy));
    }

    function passwordLabel() {
        if (!hasEntropy()) {
            return Tr.tr("Password");
        }
        return Tr.tr("Password (Entropy: %1 bits)").arg(Number(entryEntropy).toFixed(2));
    }

    onStatusChanged: {
        if (status === PageStatus.Active && openEditor) {
            openEditor = false;
            keepassrx.getEntryForEdit(entryUuid);
        }
        if (status === PageStatus.Active) {
            applicationWindow.coverEntryUuid = entryUuid;
            applicationWindow.coverEntryTitle = entryTitle && entryTitle.length > 0
                ? entryTitle
                : Tr.tr("Untitled entry");
            applicationWindow.coverEntryIcon = entryIconUrl;
            applicationWindow.coverHasUsername = entryHasUsername;
            applicationWindow.coverHasPassword = entryHasPassword;
            applicationWindow.coverHasTotp = entryHasTotp;
        }
    }

    // Values revealed on demand for fields the payload omits (hidden-by-default:
    // always the password, plus any custom field marked hidden).
    property string usernameRevealed: ""
    property string passwordRevealed: ""
    property string urlRevealed: ""
    property string notesRevealed: ""

    function revealOrHide(name, shown) {
        if (shown) {
            setRevealed(name, "");
            gc();
        } else {
            keepassrx.revealFieldValue(entryUuid, name);
        }
    }

    function copyField(name) {
        if (name === "URL") {
            keepassrx.getFieldValue(entryUuid, name);
        } else {
            keepassrx.copyFieldValue(entryUuid, name);
        }
    }

    function viewAttachment(name) {
        entryModel.darkTheme = Theme.colorScheme === Theme.LightOnDark;
        var result = entryModel.viewAttachment(name);
        if (!result.ok) {
            applicationWindow.notify(result.error
                ? result.error : Tr.tr("Unable to open attachment."));
            return;
        }
        pageStack.push(Qt.resolvedUrl("ViewAttachmentPage.qml"), {
            entryUuid: entryPage.entryUuid,
            attachmentName: name,
            displayName: result.fileName ? result.fileName : name,
            mimeType: result.mimeType ? result.mimeType : "",
            viewType: result.viewType ? result.viewType : "",
            text: result.text ? result.text : "",
            highlightedText: result.highlightedText ? result.highlightedText : "",
            dataUrl: result.dataUrl ? result.dataUrl : "",
            backgroundColor: result.backgroundColor ? result.backgroundColor : "",
            foregroundColor: result.foregroundColor ? result.foregroundColor : ""
        });
    }

    function clearRevealed() {
        revealTimer.stop();
        usernameRevealed = "";
        passwordRevealed = "";
        urlRevealed = "";
        notesRevealed = "";
        for (var i = 0; i < customFieldsModel.count; i++) {
            customFieldsModel.setProperty(i, "revealedValue", "");
        }
        gc();
    }

    readonly property bool appActive: Qt.application.state === Qt.ApplicationActive
    onAppActiveChanged: {
        if (!appActive) {
            clearRevealed();
        }
    }

    Timer {
        id: revealTimer
        interval: 30000
        onTriggered: entryPage.clearRevealed()
    }

    Component.onDestruction: clearRevealed()

    Connections {
        target: applicationWindow
        onSensitiveUiCleared: {
            entryPage.clearRevealed();
            entryTitle = "";
            entryUsername = "";
            entryUrl = "";
            entryNotes = "";
            entryCustomFields = null;
            customFieldsModel.clear();
        }
    }

    function setRevealed(name, value) {
        switch (name) {
        case "Username": usernameRevealed = value; break;
        case "Password": passwordRevealed = value; break;
        case "URL": urlRevealed = value; break;
        case "Notes": notesRevealed = value; break;
        default:
            for (var i = 0; i < customFieldsModel.count; i++) {
                if (customFieldsModel.get(i).fieldName === name) {
                    customFieldsModel.setProperty(i, "revealedValue", value);
                    break;
                }
            }
        }
    }

    function loadCustomFields() {
        customFieldsModel.clear();
        if (entryCustomFields) {
            var keys = Object.keys(entryCustomFields);
            for (var i = 0; i < keys.length; i++) {
                var f = entryCustomFields[keys[i]];
                customFieldsModel.append({
                    fieldName: keys[i],
                    plaintext: (f && f.value) ? f.value : "",
                    hiddenByDefault: !!(f && f.isHiddenByDefault),
                    revealedValue: ""
                });
            }
        }
    }

    function applyEntry(entry) {
        entryTitle = entry.title ? entry.title : "";
        entryUsername = entry.username ? entry.username : "";
        entryUrl = entry.url ? entry.url : "";
        entryNotes = entry.notes ? entry.notes : "";
        entryHasUsername = entry.hasUsername === true;
        entryHasPassword = entry.hasPassword === true;
        entryEntropy = entry.entropy !== undefined ? entry.entropy : null;
        entryHasUrl = entry.hasUrl === true;
        entryHasNotes = entry.hasNotes === true;
        entryHasTotp = entry.hasTotp === true;
        entryCustomFields = entry.customFields ? entry.customFields : null;
        entryIconUrl = entry.iconPath ? entry.iconPath : "";
        usernameRevealed = "";
        passwordRevealed = "";
        urlRevealed = "";
        notesRevealed = "";
        loadCustomFields();
    }

    Component.onCompleted: loadCustomFields()

    RxUiEntry {
        id: entryModel
        entryUuid: entryPage.entryUuid
        app: AppState
        onReadyChanged: if (ready) entryModel.loadAttachments()
        Component.onCompleted: if (ready) entryModel.loadAttachments()
    }

    Timer {
        interval: 1000
        repeat: true
        running: entryHasTotp && entryPage.entryUuid.length > 0
        triggeredOnStart: true
        onTriggered: if (entryHasTotp) entryModel.updateTotp()
    }

    Connections {
        target: keepassrx

        onEntryForEditReceived: {
            if (entry && entry.uuid === entryPage.entryUuid) {
                pageStack.push(Qt.resolvedUrl("EditEntryPage.qml"), { entry: entry });
            }
        }

        onEntrySaved: {
            if (entryUuid === entryPage.entryUuid) {
                keepassrx.getSingleEntry(entryPage.entryUuid);
            }
        }

        onSingleEntryReceived: {
            if (entry && entry.uuid === entryPage.entryUuid) {
                entryPage.applyEntry(entry);
            }
        }

        onFieldValueReceived: {
            if (field_extra !== "reveal" || entry_uuid !== entryPage.entryUuid) {
                return;
            }
            revealTimer.restart();
            entryPage.setRevealed(field_name, field_value);
        }

        onDatabaseClosed: entryPage.clearRevealed()
        onMasterPasswordInvalidated: entryPage.clearRevealed()
    }

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: content.height

        VerticalScrollDecorator {}

        PullDownMenu {
            MenuItem {
                text: Tr.tr("Edit")
                visible: keepassrx.canEdit
                onClicked: keepassrx.getEntryForEdit(entryPage.entryUuid)
            }
        }

        Column {
            id: content
            width: parent.width

            PageHeader {
                title: entryTitle && entryTitle.length > 0
                    ? entryTitle
                    : Tr.tr("Untitled entry")
            }

            SectionHeader {
                text: Tr.tr("Main")
                visible: entryHasUsername || entryHasPassword || entryHasUrl || entryHasTotp
            }

            DetailField {
                visible: entryHasUsername
                label: Tr.tr("Username")
                sensitive: entryUsername.length === 0
                revealed: usernameRevealed.length > 0
                value: entryUsername.length > 0 ? entryUsername : usernameRevealed
                onToggle: entryPage.revealOrHide("Username", usernameRevealed.length > 0)
                onCopy: entryPage.copyField("Username")
            }

            DetailField {
                visible: entryHasPassword
                label: entryPage.passwordLabel()
                sensitive: true
                revealed: passwordRevealed.length > 0
                value: passwordRevealed
                onToggle: entryPage.revealOrHide("Password", passwordRevealed.length > 0)
                onCopy: entryPage.copyField("Password")
            }

            DetailField {
                visible: entryHasUrl
                label: Tr.tr("URL")
                sensitive: entryUrl.length === 0
                revealed: urlRevealed.length > 0
                value: entryUrl.length > 0 ? entryUrl : urlRevealed
                onToggle: entryPage.revealOrHide("URL", urlRevealed.length > 0)
                onCopy: entryPage.copyField("URL")
            }

            DetailField {
                visible: entryHasTotp
                label: entryModel.currentTotpValidFor.length > 0
                    ? Tr.tr("TOTP (valid for %1)").arg(entryModel.currentTotpValidFor)
                    : Tr.tr("TOTP")
                sensitive: false
                value: entryModel.currentTotp
                onCopy: keepassrx.getTotp(entryUuid)
            }

            SectionHeader {
                text: Tr.tr("Notes")
                visible: entryHasNotes
            }

            DetailField {
                visible: entryHasNotes
                label: Tr.tr("Notes")
                sensitive: entryNotes.length === 0
                revealed: notesRevealed.length > 0
                value: entryNotes.length > 0 ? entryNotes : notesRevealed
                onToggle: entryPage.revealOrHide("Notes", notesRevealed.length > 0)
                onCopy: entryPage.copyField("Notes")
            }

            SectionHeader {
                text: Tr.tr("Other fields")
                visible: customFieldsModel.count > 0
            }

            Repeater {
                model: ListModel { id: customFieldsModel }

                DetailField {
                    label: fieldName
                    sensitive: hiddenByDefault && plaintext.length === 0
                    revealed: revealedValue.length > 0
                    value: plaintext.length > 0 ? plaintext : revealedValue
                    onToggle: entryPage.revealOrHide(fieldName, revealedValue.length > 0)
                    onCopy: entryPage.copyField(fieldName)
                }
            }

            SectionHeader {
                text: Tr.tr("Attachments")
                visible: entryModel.attachmentCount > 0
            }

            Repeater {
                model: entryModel.attachments

                AttachmentItem {
                    attachmentName: model.attachmentName
                    attachmentSize: model.attachmentSize
                    attachmentMimeType: model.attachmentMimeType
                    onActivated: entryPage.viewAttachment(name)
                }
            }
        }
    }
}
