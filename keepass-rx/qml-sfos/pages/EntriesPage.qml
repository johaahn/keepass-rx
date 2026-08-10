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

import "../components"

Page {
    id: entriesPage
    allowedOrientations: defaultAllowedOrientations

    function getEntries(containerUuid) {
        keepassrx.getEntries(containerUuid, searchField.text);
    }

    function headerTitle() {
        if (containerStack.containerName && !containerStack.isAtRoot) {
            return containerStack.containerName;
        }
        return "KeePassRX";
    }

    Component.onCompleted: {
        keepassrx.getMetadata();
        // The container stack initialises from the current view on creation and
        // emits containerChanged (handled below). Kick off an initial load only
        // if we already have a valid container, to avoid a spurious empty-UUID
        // error before that signal arrives.
        if (containerStack.containerUuid) {
            getEntries(containerStack.containerUuid);
        }
    }

    RxUiContainerStack {
        id: containerStack
        app: AppState
        viewMode: keepassrx.viewMode

        onContainerChanged: {
            searchField.text = '';
            entriesModel.clear();
            entriesPage.getEntries(container_uuid);
        }
    }

    Connections {
        target: keepassrx

        onEntriesReceived: {
            entriesModel.clear();
            for (var i = 0; i < entries.length; i++) {
                entriesModel.append({ entryUuid: entries[i] });
            }
        }

        // Full entry loaded: open the detail page.
        onSingleEntryReceived: {
            if (!entry) {
                return;
            }
            pageStack.push(Qt.resolvedUrl("EntryDetailsPage.qml"), {
                entryUuid: entry.uuid ? entry.uuid : "",
                entryTitle: entry.title ? entry.title : "",
                entryUsername: entry.username ? entry.username : "",
                entryPassword: entry.password ? entry.password : "",
                entryUrl: entry.url ? entry.url : "",
                entryNotes: entry.notes ? entry.notes : "",
                entryHasUsername: entry.hasUsername === true,
                entryHasPassword: entry.hasPassword === true,
                entryHasUrl: entry.hasUrl === true,
                entryHasNotes: entry.hasNotes === true,
                entryHasTotp: entry.hasTotp === true,
                entryCustomFields: entry.customFields ? entry.customFields : null
            });
        }
    }

    ListModel {
        id: entriesModel
    }

    SilicaListView {
        id: entriesList
        anchors.fill: parent
        model: entriesModel

        header: Column {
            width: entriesList.width

            PageHeader {
                title: entriesPage.headerTitle()
            }

            SearchField {
                id: searchField
                width: parent.width
                //% "Search entries"
                placeholderText: qsTrId("keepassrx-search-entries")
                inputMethodHints: Qt.ImhNoPredictiveText
                onTextChanged: entriesPage.getEntries(containerStack.containerUuid)
            }
        }

        PullDownMenu {
            MenuItem {
                //% "Close database"
                text: qsTrId("keepassrx-close-database")
                onClicked: applicationWindow.closeDatabase()
            }
            MenuItem {
                //% "Settings"
                text: qsTrId("keepassrx-settings")
                onClicked: pageStack.push(Qt.resolvedUrl("SettingsPage.qml"))
            }
            MenuItem {
                //% "Go up"
                text: qsTrId("keepassrx-go-up")
                visible: !containerStack.isAtRoot
                onClicked: containerStack.popContainer()
            }
        }

        ViewPlaceholder {
            enabled: entriesModel.count === 0
            //% "No entries"
            text: qsTrId("keepassrx-no-entries")
        }

        delegate: EntryListItem {
            uuid: entryUuid
            onGroupActivated: containerStack.pushContainer(uuid)
            onEntryActivated: keepassrx.getSingleEntry(uuid)
        }

        VerticalScrollDecorator {}
    }
}
