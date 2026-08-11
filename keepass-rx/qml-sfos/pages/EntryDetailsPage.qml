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
    property bool entryHasUrl: false
    property bool entryHasNotes: false
    property bool entryHasTotp: false
    property var entryCustomFields: null

    // Values revealed on demand for fields the payload omits (hidden-by-default:
    // always the password, plus any custom field marked hidden).
    property string usernameRevealed: ""
    property string passwordRevealed: ""
    property string urlRevealed: ""
    property string notesRevealed: ""

    function revealOrHide(name, shown) {
        if (shown) {
            setRevealed(name, "");
        } else {
            keepassrx.revealFieldValue(entryUuid, name);
        }
    }

    function copyField(name) {
        keepassrx.getFieldValue(entryUuid, name);
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

    Component.onCompleted: {
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

    Connections {
        target: keepassrx
        onFieldValueReceived: {
            if (field_extra !== "reveal" || entry_uuid !== entryPage.entryUuid) {
                return;
            }
            entryPage.setRevealed(field_name, field_value);
        }
    }

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: content.height

        VerticalScrollDecorator {}

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
                label: Tr.tr("Password")
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
                label: Tr.tr("TOTP")
                sensitive: false
                value: Tr.tr("Long-press to copy the 2FA code")
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
        }
    }
}
