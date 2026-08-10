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
    property string entryPassword: ""
    property string entryUrl: ""
    property string entryNotes: ""
    property bool entryHasUsername: false
    property bool entryHasPassword: false
    property bool entryHasUrl: false
    property bool entryHasNotes: false
    property bool entryHasTotp: false
    property var entryCustomFields: null

    function copyField(fieldName) {
        keepassrx.getFieldValue(entryUuid, fieldName);
    }

    Component.onCompleted: {
        if (entryCustomFields) {
            var keys = Object.keys(entryCustomFields);
            for (var i = 0; i < keys.length; i++) {
                customFieldsModel.append({ fieldName: keys[i] });
            }
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

            // ------- Main fields -------
            SectionHeader {
                text: Tr.tr("Main")
                visible: entryHasUsername || entryHasPassword || entryHasUrl || entryHasTotp
            }

            CopyField {
                label: Tr.tr("Username")
                value: entryUsername.length > 0 ? entryUsername : "••••••"
                visible: entryHasUsername
                onClicked: entryPage.copyField("Username")
            }

            CopyField {
                label: Tr.tr("Password")
                value: "••••••••"
                visible: entryHasPassword
                onClicked: entryPage.copyField("Password")
            }

            CopyField {
                label: Tr.tr("URL")
                value: entryUrl.length > 0 ? entryUrl : "••••••"
                visible: entryHasUrl
                onClicked: entryPage.copyField("URL")
            }

            CopyField {
                label: Tr.tr("TOTP")
                value: Tr.tr("Tap to copy 2FA code")
                visible: entryHasTotp
                onClicked: keepassrx.getTotp(entryUuid)
            }

            // ------- Notes -------
            SectionHeader {
                text: Tr.tr("Notes")
                visible: entryHasNotes && entryNotes.length > 0
            }

            Label {
                visible: entryHasNotes && entryNotes.length > 0
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.Wrap
                color: Theme.secondaryColor
                text: entryNotes
            }

            // ------- Custom fields -------
            SectionHeader {
                text: Tr.tr("Other fields")
                visible: customFieldsModel.count > 0
            }

            Repeater {
                model: ListModel { id: customFieldsModel }

                CopyField {
                    label: fieldName
                    value: "••••••"
                    onClicked: entryPage.copyField(fieldName)
                }
            }
        }
    }
}
