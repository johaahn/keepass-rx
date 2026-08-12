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
    id: licensesPage
    allowedOrientations: defaultAllowedOrientations

    RxUiLicenses {
        id: uiLicenses
    }

    ListModel {
        id: licenseListModel
    }

    Component.onCompleted: {
        var licenses = uiLicenses.allLicenses();
        for (var i = 0; i < licenses.length; i++) {
            licenseListModel.append(licenses[i]);
        }
    }

    SilicaListView {
        id: licenseList
        anchors.fill: parent
        model: licenseListModel

        header: PageHeader {
            title: Tr.tr("Open Source Licenses")
        }

        delegate: ListItem {
            id: delegate
            contentHeight: Theme.itemSizeMedium

            Column {
                anchors.verticalCenter: parent.verticalCenter
                anchors.left: parent.left
                anchors.leftMargin: Theme.horizontalPageMargin
                anchors.right: parent.right
                anchors.rightMargin: Theme.horizontalPageMargin

                Label {
                    text: "%1 %2".arg(crateName).arg(crateVersion)
                    width: parent.width
                    truncationMode: TruncationMode.Fade
                    color: delegate.highlighted ? Theme.highlightColor : Theme.primaryColor
                }
                Label {
                    text: licenseName
                    width: parent.width
                    truncationMode: TruncationMode.Fade
                    color: delegate.highlighted ? Theme.secondaryHighlightColor : Theme.secondaryColor
                    font.pixelSize: Theme.fontSizeExtraSmall
                }
            }

            onClicked: pageStack.push(Qt.resolvedUrl("LicenseTextPage.qml"), {
                projectName: "%1 %2".arg(crateName).arg(crateVersion),
                licenseText: licenseText
            })

            menu: ContextMenu {
                MenuItem {
                    text: Tr.tr("Website")
                    visible: crateURL.length > 0
                    onClicked: Qt.openUrlExternally(crateURL)
                }
            }
        }

        VerticalScrollDecorator {}
    }
}
