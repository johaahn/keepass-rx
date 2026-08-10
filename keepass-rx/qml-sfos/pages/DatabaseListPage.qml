/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation; version 3.
 */
import QtQuick 2.6
import Sailfish.Silica 1.0
import Sailfish.Pickers 1.0
import keepassrx 1.0

Page {
    id: dbListPage
    allowedOrientations: defaultAllowedOrientations

    Component {
        id: filePicker
        FilePickerPage {
            title: Tr.tr("Select database")
            nameFilters: [ '*.kdbx', '*.kdb' ]
            onSelectedContentPropertiesChanged: {
                keepassrx.importDatabase(selectedContentProperties.filePath);
            }
        }
    }

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
                text: Tr.tr("Settings")
                onClicked: pageStack.push(Qt.resolvedUrl("SettingsPage.qml"))
            }
            MenuItem {
                text: Tr.tr("Refresh")
                onClicked: {
                    dbListModel.clear();
                    keepassrx.listImportedDatabases();
                }
            }
            MenuItem {
                text: Tr.tr("Add database")
                onClicked: pageStack.push(filePicker)
            }
        }

        ViewPlaceholder {
            enabled: dbListModel.count === 0
            text: Tr.tr("No databases")
            hintText: Tr.tr("Pull down to add a database.")
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
                    text: Tr.tr("Last modified: %1").arg(formatLastModified(lastModified))
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
                    text: Tr.tr("Delete")
                    visible: databaseTypeString === 'Imported'
                    onClicked: {
                        delegate.remorseAction(Tr.tr("Deleting"), function() {
                            keepassrx.deleteDatabase(databaseName);
                        });
                    }
                }
            }
        }

        VerticalScrollDecorator {}
    }
}
