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
    id: licenseTextPage
    allowedOrientations: defaultAllowedOrientations

    property string projectName
    property string licenseText

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: content.height

        VerticalScrollDecorator {}

        Column {
            id: content
            width: parent.width
            bottomPadding: Theme.paddingLarge

            PageHeader {
                title: licenseTextPage.projectName
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                text: licenseTextPage.licenseText
                wrapMode: Text.Wrap
                color: Theme.primaryColor
                font.pixelSize: Theme.fontSizeExtraSmall
                font.family: "monospace"
            }
        }
    }
}
