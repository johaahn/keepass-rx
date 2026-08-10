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
    id: dbListPage
    allowedOrientations: defaultAllowedOrientations

    function formatLastModified(lastModified) {
        return Qt.formatDateTime(lastModified,
                                 Qt.locale().dateTimeFormat(Locale.ShortFormat));
    }

    Component.onCompleted: {
        if (keepassrx.databaseOpen) {
            applicationWindow.uiDatabase.clearKeyFile();
            keepassrx.closeDatabase();
        }
        dbListModel.clear();
        keepassrx.listImportedDatabases();
    }

    Connections {
        target: keepassrx

        onDatabaseImported: {
            dbListModel.append({
                databaseName: db_name,
                databaseTypeString: db_type.toString(),
                lastModified: last_modified
            });
        }

        onDatabaseDeleted: {
            for (var i = 0; i < dbListModel.count; i++) {
                if (dbListModel.get(i).databaseName === db_name) {
                    dbListModel.remove(i);
                    break;
                }
            }
        }
    }

    ListModel {
        id: dbListModel
        dynamicRoles: true
    }

    SilicaListView {
        id: dbList
        anchors.fill: parent
        model: dbListModel

        header: PageHeader {
            title: "KeePassRX"
        }

        PullDownMenu {
            MenuItem {
                //% "Settings"
                text: qsTrId("keepassrx-settings")
                onClicked: pageStack.push(Qt.resolvedUrl("SettingsPage.qml"))
            }
            MenuItem {
                //% "Refresh"
                text: qsTrId("keepassrx-refresh")
                onClicked: {
                    dbListModel.clear();
                    keepassrx.listImportedDatabases();
                }
            }
        }

        ViewPlaceholder {
            enabled: dbListModel.count === 0
            //% "No databases"
            text: qsTrId("keepassrx-no-databases")
            //% "Copy a .kdbx file into the app's data directory to get started."
            hintText: qsTrId("keepassrx-no-databases-hint")
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
                    text: databaseName
                    width: parent.width
                    truncationMode: TruncationMode.Fade
                    color: delegate.highlighted ? Theme.highlightColor : Theme.primaryColor
                }
                Label {
                    //% "Last modified: %1"
                    text: qsTrId("keepassrx-last-modified").arg(formatLastModified(lastModified))
                    width: parent.width
                    truncationMode: TruncationMode.Fade
                    color: delegate.highlighted ? Theme.secondaryHighlightColor : Theme.secondaryColor
                    font.pixelSize: Theme.fontSizeExtraSmall
                }
            }

            onClicked: {
                applicationWindow.uiDatabase.databaseName = databaseName;
                applicationWindow.uiDatabase.databaseType = databaseTypeString;
                pageStack.push(Qt.resolvedUrl("OpenDatabasePage.qml"));
            }

            menu: ContextMenu {
                MenuItem {
                    //% "Delete"
                    text: qsTrId("keepassrx-delete")
                    visible: databaseTypeString === 'Imported'
                    onClicked: {
                        //% "Deleting"
                        delegate.remorseAction(qsTrId("keepassrx-deleting"), function() {
                            keepassrx.deleteDatabase(databaseName);
                        });
                    }
                }
            }
        }

        VerticalScrollDecorator {}
    }
}
