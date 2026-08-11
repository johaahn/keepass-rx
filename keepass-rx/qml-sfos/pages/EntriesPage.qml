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

    property string containerUuid: ""
    property string containerTitle: ""
    property bool isViewRoot: containerUuid.length === 0
    property string searchTerm: ""
    property bool loaded: false

    function currentUuid() {
        return isViewRoot ? containerStack.containerUuid : containerUuid;
    }

    function viewLabel(mode) {
        switch (mode) {
        case "Templates": return Tr.tr("Special categories");
        case "Totp": return Tr.tr("2FA codes");
        case "Tags": return Tr.tr("Tags");
        case "SavedSearches": return Tr.tr("Saved searches");
        default: return "KeePassRX";
        }
    }

    function headerTitle() {
        return isViewRoot ? viewLabel(keepassrx.viewMode) : containerTitle;
    }

    function loadEntries() {
        var uuid = currentUuid();
        if (uuid.length > 0) {
            keepassrx.getEntries(uuid, searchTerm);
        }
    }

    // The container stack is the navigation business logic. Only the view
    // root page drives it; child pages resolve their own container by the
    // uuid passed in when the group was tapped.
    RxUiContainerStack {
        id: containerStack
        app: AppState
        viewMode: keepassrx.viewMode
        onContainerChanged: {
            if (entriesPage.isViewRoot) {
                entriesPage.loadEntries();
            }
        }
    }

    Component.onCompleted: keepassrx.getMetadata()

    onStatusChanged: {
        if (status === PageStatus.Active) {
            loadEntries();
        }
    }

    Connections {
        target: keepassrx

        onEntriesReceived: {
            if (entriesPage.status !== PageStatus.Active) {
                return;
            }
            entriesModel.clear();
            for (var i = 0; i < entries.length; i++) {
                entriesModel.append({ entryUuid: entries[i] });
            }
            entriesPage.loaded = true;
        }

        onSingleEntryReceived: {
            if (entriesPage.status !== PageStatus.Active || !entry) {
                return;
            }
            pageStack.push(Qt.resolvedUrl("EntryDetailsPage.qml"), {
                entryUuid: entry.uuid ? entry.uuid : "",
                entryTitle: entry.title ? entry.title : "",
                entryUsername: entry.username ? entry.username : "",
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

    Item {
        id: headerBox
        y: 0 - entriesList.contentY - height
        z: 1
        width: parent.width
        height: pageHeader.height + searchField.height

        PageHeader {
            id: pageHeader
            anchors { top: parent.top; left: parent.left }
            width: parent.width
            title: entriesPage.headerTitle()
        }

        SearchField {
            id: searchField
            anchors { top: pageHeader.bottom; left: parent.left }
            width: parent.width
            placeholderText: Tr.tr("Search entries")
            inputMethodHints: Qt.ImhNoPredictiveText
            EnterKey.iconSource: "image://theme/icon-m-enter-close"
            EnterKey.onClicked: entriesList.focus = true
            onTextChanged: {
                entriesPage.searchTerm = text;
                entriesPage.loadEntries();
            }
        }
    }

    SilicaListView {
        id: entriesList
        anchors.fill: parent
        model: entriesModel

        header: Item {
            width: parent.width
            height: headerBox.height
        }

        PullDownMenu {
            MenuItem {
                text: Tr.tr("Close Database")
                onClicked: applicationWindow.closeDatabase()
            }
            MenuItem {
                text: Tr.tr("Settings")
                onClicked: pageStack.push(Qt.resolvedUrl("SettingsPage.qml"))
            }
            MenuItem {
                text: Tr.tr("Change View")
                onClicked: pageStack.push(Qt.resolvedUrl("ViewModePage.qml"))
            }
        }

        ViewPlaceholder {
            y: 0 - entriesList.contentY
            enabled: entriesPage.loaded && entriesModel.count === 0
            text: Tr.tr("No entries")
        }

        delegate: EntryListItem {
            uuid: entryUuid
            onGroupActivated: pageStack.push(Qt.resolvedUrl("EntriesPage.qml"), {
                containerUuid: uuid,
                containerTitle: title
            })
            onEntryActivated: keepassrx.getSingleEntry(uuid)
        }

        VerticalScrollDecorator {}
    }
}
