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
    id: aboutPage
    allowedOrientations: defaultAllowedOrientations

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: content.height

        VerticalScrollDecorator {}

        PullDownMenu {
            MenuItem {
                text: Tr.tr("Open Source Licenses")
                onClicked: pageStack.push(Qt.resolvedUrl("LicensesPage.qml"))
            }
        }

        Column {
            id: content
            width: parent.width
            spacing: Theme.paddingMedium
            bottomPadding: Theme.paddingLarge

            PageHeader {
                title: Tr.tr("About")
            }

            Image {
                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width * 0.5
                height: width
                fillMode: Image.PreserveAspectFit
                source: "../../assets/keepass-rx.svg"
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "KeePassRX"
                color: Theme.highlightColor
                font.pixelSize: Theme.fontSizeExtraLarge
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Tr.tr("Version %1").arg(keepassRxVersion)
                color: Theme.secondaryColor
                font.pixelSize: Theme.fontSizeSmall
            }

            Item {
                width: parent.width
                height: Theme.paddingMedium
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.Wrap
                color: Theme.primaryColor
                font.pixelSize: Theme.fontSizeSmall
                text: Tr.tr("KeePassRX is a password manager for KeePass databases. It is licensed under the GNU AGPL v3 license.")
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.Wrap
                color: Theme.secondaryColor
                font.pixelSize: Theme.fontSizeSmall
                text: Tr.tr("The built-in KeePass icon images are licensed under a variety of licenses as detailed in assets/COPYING.")
            }

            SectionHeader {
                text: Tr.tr("Manual")
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.Wrap
                textFormat: Text.RichText
                color: Theme.primaryColor
                linkColor: Theme.highlightColor
                font.pixelSize: Theme.fontSizeSmall
                text: "• Web: <a href='https://agnos.is/projects/keepassrx/'>https://agnos.is/projects/keepassrx/</a>"
                onLinkActivated: Qt.openUrlExternally(link)
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.Wrap
                textFormat: Text.RichText
                color: Theme.primaryColor
                linkColor: Theme.highlightColor
                font.pixelSize: Theme.fontSizeSmall
                text: "• Gemini: <a href='gemini://agnos.is/projects/keepassrx/'>gemini://agnos.is/projects/keepassrx/</a>"
                onLinkActivated: Qt.openUrlExternally(link)
            }

            SectionHeader {
                text: Tr.tr("Source Code")
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.Wrap
                textFormat: Text.RichText
                color: Theme.primaryColor
                linkColor: Theme.highlightColor
                font.pixelSize: Theme.fontSizeSmall
                text: "<a href='https://git.agnos.is/projectmoon/keepass-rx'>https://git.agnos.is/projectmoon/keepass-rx</a>"
                onLinkActivated: Qt.openUrlExternally(link)
            }
        }
    }
}
