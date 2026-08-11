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

Item {
    id: totp

    property string uuid

    RxUiEntry {
        id: entry
        entryUuid: totp.uuid
        app: AppState
    }

    Timer {
        interval: 1000
        repeat: true
        running: totp.uuid.length > 0
        triggeredOnStart: true
        onTriggered: if (totp.uuid) entry.updateTotp()
    }

    Column {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width

        Label {
            width: parent.width
            horizontalAlignment: Text.AlignRight
            text: entry.currentTotp
            color: Theme.highlightColor
            font.pixelSize: Theme.fontSizeLarge
        }

        Label {
            width: parent.width
            horizontalAlignment: Text.AlignRight
            visible: text.length > 0
            text: entry.currentTotpValidFor
            color: Theme.secondaryColor
            font.pixelSize: Theme.fontSizeExtraSmall
        }
    }
}
