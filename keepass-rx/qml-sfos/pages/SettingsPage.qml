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
                title: Tr.tr("Settings")
            }

            TextSwitch {
                text: Tr.tr("Database accent colors")
                description: Tr.tr("Tint entries with the color assigned to them in the database.")
                checked: SettingsBridge.showAccents
                onClicked: SettingsBridge.showAccents = checked
            }

            TextSwitch {
                text: Tr.tr("Show recycle bin")
                description: Tr.tr("Include entries from the database's recycle bin group while browsing.")
                checked: SettingsBridge.showRecycleBin
                onClicked: SettingsBridge.showRecycleBin = checked
            }

            TextSwitch {
                text: Tr.tr("Enable database locking")
                description: Tr.tr("Offer a Lock action that locks the database with your master password instead of closing it.")
                checked: SettingsBridge.databaseLocking
                onClicked: SettingsBridge.databaseLocking = checked
            }
        }
    }
}
