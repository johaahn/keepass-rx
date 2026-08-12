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

ListItem {
    id: entryItem

    property string uuid
    readonly property string title: theEntry.title
    readonly property bool isTotp: theEntry.hasTOTP && keepassrx.viewMode == 'Totp'

    signal groupActivated()
    signal entryActivated()

    contentHeight: Theme.itemSizeMedium

    RxListItem {
        id: theEntry
        entryUuid: uuid
        app: AppState
    }

    RxUiEntry {
        id: totpEntry
        entryUuid: uuid
        app: AppState
    }

    Timer {
        interval: 1000
        repeat: true
        running: entryItem.isTotp && uuid.length > 0
        triggeredOnStart: true
        onTriggered: if (entryItem.isTotp) totpEntry.updateTotp()
    }

    function isGrouping() {
        return theEntry.itemType == 'Group'
            || theEntry.itemType == 'Template'
            || theEntry.itemType == 'Tag'
            || theEntry.itemType == 'SavedSearch';
    }

    function resolveImagePath() {
        if (theEntry.iconPath) {
            return theEntry.iconBuiltin
                ? Qt.resolvedUrl("../../assets/icons/" + theEntry.iconPath)
                : theEntry.iconPath;
        }
        return Qt.resolvedUrl("../../assets/placeholder.png");
    }

    onClicked: {
        if (isTotp) {
            keepassrx.getTotp(uuid);
        } else if (isGrouping()) {
            entryItem.groupActivated();
        } else {
            entryItem.entryActivated();
        }
    }

    Image {
        id: icon
        anchors.verticalCenter: parent.verticalCenter
        x: Theme.horizontalPageMargin
        width: Theme.iconSizeMedium
        height: Theme.iconSizeMedium
        fillMode: Image.PreserveAspectFit
        source: theEntry.ready ? resolveImagePath()
                               : Qt.resolvedUrl("../../assets/placeholder.png")
    }

    Column {
        id: totpColumn
        visible: entryItem.isTotp
        width: Theme.itemSizeMedium
        anchors {
            right: parent.right
            rightMargin: Theme.horizontalPageMargin
            verticalCenter: parent.verticalCenter
        }

        Label {
            width: parent.width
            horizontalAlignment: Text.AlignRight
            text: totpEntry.currentTotp
            color: Theme.highlightColor
            font.pixelSize: Theme.fontSizeLarge
        }

        Label {
            width: parent.width
            horizontalAlignment: Text.AlignRight
            visible: text.length > 0
            text: totpEntry.currentTotpValidFor
            color: Theme.secondaryColor
            font.pixelSize: Theme.fontSizeExtraSmall
        }
    }

    Column {
        anchors {
            left: icon.right
            leftMargin: Theme.paddingMedium
            right: totpColumn.visible ? totpColumn.left : parent.right
            rightMargin: Theme.paddingMedium
            verticalCenter: parent.verticalCenter
        }

        Label {
            width: parent.width
            truncationMode: TruncationMode.Fade
            text: theEntry.title
            color: entryItem.highlighted ? Theme.highlightColor : Theme.primaryColor
        }

        Label {
            width: parent.width
            truncationMode: TruncationMode.Fade
            visible: text.length > 0
            text: entryItem.isTotp ? Tr.tr("Tap to copy 2FA code") : theEntry.subtitle
            font.pixelSize: Theme.fontSizeExtraSmall
            color: entryItem.highlighted ? Theme.secondaryHighlightColor : Theme.secondaryColor
        }
    }

    menu: ContextMenu {
        MenuItem {
            text: Tr.tr("View details")
            visible: entryItem.isTotp
            onClicked: entryItem.entryActivated()
        }
        MenuItem {
            text: Tr.tr("Copy username")
            visible: theEntry.hasUsername
            onClicked: keepassrx.getFieldValue(uuid, "Username")
        }
        MenuItem {
            text: Tr.tr("Copy password")
            visible: theEntry.hasPassword
            onClicked: keepassrx.getFieldValue(uuid, "Password")
        }
        MenuItem {
            text: Tr.tr("Open URL")
            visible: theEntry.hasURL
            onClicked: keepassrx.getFieldValue(uuid, "URL")
        }
        MenuItem {
            text: Tr.tr("Copy 2FA code")
            visible: theEntry.hasTOTP
            onClicked: keepassrx.getTotp(uuid)
        }
    }
}
