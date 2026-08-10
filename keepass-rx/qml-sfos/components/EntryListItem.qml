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

    signal groupActivated()
    signal entryActivated()

    contentHeight: Theme.itemSizeMedium

    RxListItem {
        id: theEntry
        entryUuid: uuid
        app: AppState
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
        if (isGrouping()) {
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
        anchors {
            left: icon.right
            leftMargin: Theme.paddingMedium
            right: parent.right
            rightMargin: Theme.horizontalPageMargin
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
            text: theEntry.subtitle
            font.pixelSize: Theme.fontSizeExtraSmall
            color: entryItem.highlighted ? Theme.secondaryHighlightColor : Theme.secondaryColor
        }
    }

    menu: ContextMenu {
        MenuItem {
            //% "Copy username"
            text: qsTrId("keepassrx-copy-username")
            visible: theEntry.hasUsername
            onClicked: keepassrx.getFieldValue(uuid, "Username")
        }
        MenuItem {
            //% "Copy password"
            text: qsTrId("keepassrx-copy-password")
            visible: theEntry.hasPassword
            onClicked: keepassrx.getFieldValue(uuid, "Password")
        }
        MenuItem {
            //% "Open URL"
            text: qsTrId("keepassrx-open-url")
            visible: theEntry.hasURL
            onClicked: keepassrx.getFieldValue(uuid, "URL")
        }
        MenuItem {
            //% "Copy 2FA code"
            text: qsTrId("keepassrx-copy-totp")
            visible: theEntry.hasTOTP
            onClicked: keepassrx.getTotp(uuid)
        }
    }
}
