/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation; version 3.
 */
import QtQuick 2.6
import Sailfish.Silica 1.0

CoverBackground {
    id: cover

    property string guiState: ""
    property string dbName: ""
    property string entryTitle: ""
    property string entryIcon: ""
    property string groupTitle: ""
    property bool hasPassword: false
    property bool hasTotp: false
    property bool hasUsername: false

    signal requestCopy(string kind)
    signal requestUnlock()

    property string infoText: ""
    readonly property bool showingInfo: infoText.length > 0

    readonly property bool locked: guiState === 'Locked'
    readonly property bool inEntry: guiState === 'Open' && entryTitle.length > 0

    // Priority order (password, TOTP, username), capped at two cover slots.
    readonly property var entryActions: {
        var actions = [];
        if (!inEntry) {
            return actions;
        }
        if (hasPassword) {
            actions.push("password");
        }
        if (hasTotp) {
            actions.push("totp");
        }
        if (hasUsername) {
            actions.push("username");
        }
        return actions.slice(0, 2);
    }

    readonly property string coverText: {
        if (locked) {
            return Tr.tr("Database locked");
        }
        if (inEntry) {
            return entryTitle;
        }
        if (guiState === 'Open') {
            return groupTitle.length > 0 ? groupTitle : "KeePassRX";
        }
        return "KeePassRX";
    }

    function actionIcon(kind) {
        switch (kind) {
        case "password": return "image://theme/icon-m-clipboard";
        case "totp": return "image://theme/icon-m-timer";
        case "username": return "image://theme/icon-m-contact";
        default: return "";
        }
    }

    function copyLabel(kind) {
        switch (kind) {
        case "password": return Tr.tr("Password copied");
        case "totp": return Tr.tr("2FA code copied");
        case "username": return Tr.tr("Username copied");
        default: return "";
        }
    }

    function copyAction(kind) {
        cover.infoText = cover.copyLabel(kind);
        infoTimer.restart();
        cover.requestCopy(kind);
    }

    Timer {
        id: infoTimer
        interval: 1500
        onTriggered: cover.infoText = ""
    }

    Label {
        id: dbLabel
        anchors {
            top: parent.top
            topMargin: Theme.paddingMedium
            horizontalCenter: parent.horizontalCenter
        }
        width: parent.width - 2 * Theme.paddingMedium
        horizontalAlignment: Text.AlignHCenter
        truncationMode: TruncationMode.Fade
        visible: cover.guiState !== 'NotOpen' && cover.dbName.length > 0
        text: cover.dbName
        color: Theme.secondaryColor
        font.pixelSize: Theme.fontSizeExtraSmall
    }

    Column {
        anchors.centerIn: parent
        width: parent.width - 2 * Theme.paddingMedium
        spacing: Theme.paddingMedium

        Image {
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width * 0.5
            height: width
            fillMode: Image.PreserveAspectFit
            source: cover.inEntry && cover.entryIcon.length > 0
                ? cover.entryIcon
                : "../../assets/keepass-rx.svg"
        }

        Label {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            maximumLineCount: 3
            truncationMode: TruncationMode.Fade
            text: cover.showingInfo ? cover.infoText : cover.coverText
            color: (cover.locked || cover.showingInfo) ? Theme.highlightColor : Theme.primaryColor
            font.pixelSize: Theme.fontSizeSmall
        }
    }

    CoverActionList {
        enabled: cover.locked

        CoverAction {
            iconSource: "image://theme/icon-m-device-lock"
            onTriggered: cover.requestUnlock()
        }
    }

    CoverActionList {
        enabled: !cover.locked && !cover.showingInfo && cover.entryActions.length === 2

        CoverAction {
            iconSource: cover.actionIcon(cover.entryActions[0])
            onTriggered: cover.copyAction(cover.entryActions[0])
        }
        CoverAction {
            iconSource: cover.actionIcon(cover.entryActions[1])
            onTriggered: cover.copyAction(cover.entryActions[1])
        }
    }

    CoverActionList {
        enabled: !cover.locked && !cover.showingInfo && cover.entryActions.length === 1

        CoverAction {
            iconSource: cover.actionIcon(cover.entryActions[0])
            onTriggered: cover.copyAction(cover.entryActions[0])
        }
    }
}
