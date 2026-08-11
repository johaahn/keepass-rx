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

Page {
    id: page
    allowedOrientations: defaultAllowedOrientations

    property var views: [
        { mode: "All",           label: Tr.tr("All entries"),        description: Tr.tr("All groups and entries.") },
        { mode: "Templates",     label: Tr.tr("Special categories"), description: Tr.tr("Entries grouped by template.") },
        { mode: "Totp",          label: Tr.tr("2FA codes"),          description: Tr.tr("Entries with 2FA codes.") },
        { mode: "Tags",          label: Tr.tr("Tags"),               description: Tr.tr("Entries grouped by tag.") },
        { mode: "SavedSearches", label: Tr.tr("Saved searches"),     description: Tr.tr("Entries matching search queries.") }
    ]

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: content.height

        VerticalScrollDecorator {}

        Column {
            id: content
            width: parent.width

            PageHeader {
                title: Tr.tr("Change View")
            }

            Repeater {
                model: page.views

                ListItem {
                    id: item
                    contentHeight: Theme.itemSizeMedium

                    property bool current: modelData.mode === keepassrx.viewMode

                    onClicked: {
                        keepassrx.viewMode = modelData.mode;
                        pageStack.pop();
                    }

                    Column {
                        anchors {
                            left: parent.left
                            leftMargin: Theme.horizontalPageMargin
                            right: check.left
                            rightMargin: Theme.paddingMedium
                            verticalCenter: parent.verticalCenter
                        }

                        Label {
                            width: parent.width
                            text: modelData.label
                            color: (item.highlighted || item.current) ? Theme.highlightColor : Theme.primaryColor
                        }
                        Label {
                            width: parent.width
                            text: modelData.description
                            wrapMode: Text.Wrap
                            font.pixelSize: Theme.fontSizeExtraSmall
                            color: (item.highlighted || item.current) ? Theme.secondaryHighlightColor : Theme.secondaryColor
                        }
                    }

                    Image {
                        id: check
                        visible: item.current
                        anchors {
                            right: parent.right
                            rightMargin: Theme.horizontalPageMargin
                            verticalCenter: parent.verticalCenter
                        }
                        source: "image://theme/icon-m-accept"
                    }
                }
            }
        }
    }
}
