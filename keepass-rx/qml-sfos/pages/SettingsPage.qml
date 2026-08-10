/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation; version 3.
 */
import QtQuick 2.6
import Sailfish.Silica 1.0

Page {
    id: settingsPage
    allowedOrientations: defaultAllowedOrientations

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: content.height

        VerticalScrollDecorator {}

        Column {
            id: content
            width: parent.width

            PageHeader {
                //% "Settings"
                title: qsTrId("keepassrx-settings")
            }

            TextSwitch {
                //% "Database accent colours"
                text: qsTrId("keepassrx-setting-accents")
                checked: SettingsBridge.showAccents
                onClicked: SettingsBridge.showAccents = checked
            }

            TextSwitch {
                //% "Show recycle bin"
                text: qsTrId("keepassrx-setting-recycle-bin")
                checked: SettingsBridge.showRecycleBin
                onClicked: SettingsBridge.showRecycleBin = checked
            }

            TextSwitch {
                //% "Enable database locking"
                text: qsTrId("keepassrx-setting-locking")
                checked: SettingsBridge.databaseLocking
                onClicked: SettingsBridge.databaseLocking = checked
            }
        }
    }
}
