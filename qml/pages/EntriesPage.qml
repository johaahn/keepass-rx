import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.3
import Lomiri.Components 1.3 as UITK
import Qt.labs.settings 1.0

import "../components"

UITK.Page {
    Settings {
        id: settings
        property bool fetchOnOpen: false
        property bool showRecycleBin: false
        property bool changeGroupOnSearch: true
        property bool databaseLocking: true
    }

    property string parentGroupUuid
    property string previousGroupUuid
    property string groupUuid
    property string groupName
    property bool searchMode: false

    onGroupUuidChanged: {
        if (groupUuid && (previousGroupUuid && groupUuid != previousGroupUuid)) {
	    populate();
        }
    }

    function popGroup() {
	if (parentGroupUuid) {
            groupUuid = parentGroupUuid;
	} else {
	    console.log('Cannot move up from root group!');
	}
    }

    function lockDatabase() {
        keepassrx.closeDatabase();
        groupUuid = null;
        groupName = null;
        root.lockUI();
    }

    function closeDatabase() {
        keepassrx.invalidateMasterPassword();
        keepassrx.closeDatabase();
        groupUuid = null;
        groupName = null;
        root.closeUI();
    }

    function isAtRoot() {
	return !parentGroupUuid || groupUuid == parentGroupUuid
    }

    header: UITK.PageHeader {
        id: header
        title: groupName && parentGroupUuid ? groupName : "KeePassRX"

        leadingActionBar.actions: [
            UITK.Action {
                enabled: settings.databaseLocking && isAtRoot()
                visible: settings.databaseLocking && isAtRoot()
                name: "Lock"
                //TRANSLATORS: Securely lock (NOT close) an open database.
                text: i18n.tr("Lock")
                iconName: "lock"
                onTriggered: {
                    lockDatabase();
                }
            },

            UITK.Action {
                enabled: !settings.databaseLocking && isAtRoot()
                visible: !settings.databaseLocking && isAtRoot()
                name: "Close"
                //TRANSLATORS: Securely close (NOT lock) an open database.
                text: i18n.tr("Close")
                iconName: "close"
                onTriggered: {
                    closeDatabase();
                }
            },

	    UITK.Action {
                enabled: !isAtRoot()
                visible: !isAtRoot()
                name: "Go Back"
                //TRANSLATORS: Move back up in the group folder structure.
                text: i18n.tr("Back")
                iconName: "back"
                onTriggered: {
                    popGroup();
                }
            }
        ]

        trailingActionBar.numberOfSlots: 2
        trailingActionBar.actions: [
            UITK.Action {
                name: "Search"
                // TRANSLATORS: Initiate (or stop) the search action.
                text: !searchMode ? i18n.tr("Search") : i18n.tr("Cancel Search")
                iconName: !searchMode ? "search" : "close"
                onTriggered: {
                    searchField.text = '';
                    searchMode = !searchMode;
                    if (searchMode) {
                        searchField.focus = true;
                    }
                }
            },

            UITK.Action {
                name: "Settings"
                text: i18n.tr("Settings")
                iconName: "settings"
                onTriggered: {
                    pageStack.addPageToNextColumn(adaptiveLayout.primaryPage, settingsPage)
                }
            },

            UITK.Action {
                name: "About"
                text: i18n.tr("About")
                iconName: "info"
                onTriggered: {
                    pageStack.addPageToNextColumn(adaptiveLayout.primaryPage, aboutPage)
                }
            },

            UITK.Action {
                name: "Lock"
                enabled: settings.databaseLocking
                visible: settings.databaseLocking
                // TRANSLATORS: Lock (NOT close) an open database.
                text: i18n.tr('Lock Database')
                iconName: "lock"
                onTriggered: {
                    lockDatabase();
                }
            },

            UITK.Action {
                name: "Close"
                // TRANSLATORS: Close (NOT lock) an open database.
                text: i18n.tr('Close Database')
                iconName: "close"
                onTriggered: {
                    closeDatabase();
                }
            }
        ]

        extension: ColumnLayout {
            id: opsBar
            Layout.fillWidth: true

            anchors {
                margins: units.gu(1)
                left: parent.left
                right: parent.right
            }

            RowLayout {
                id: searchBar
                Layout.fillWidth: true
                width: parent.width

                UITK.TextField {
                    width: parent.width
                    Layout.fillWidth: true
                    visible: searchMode
                    id: searchField
                    // TRANSLATORS: Placeholder text of the search box for searching for database entries. Group is a group/folder of password manager entries.
                    placeholderText: i18n.tr("Search entries in this group")
                    inputMethodHints: Qt.ImhNoPredictiveText
                    onTextChanged: {
                        getEntries(groupUuid);
                    }
                }
            }
        }
    }

    ListView {
	id: entriesList
        clip: true
        z: 1
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        spacing: units.gu(0.1)

        model: ListModel {
            id: entriesListModel
        }

      delegate: EntryItem {}
    }

    Popup {
        id: toast
        padding: units.dp(12)

        x: parent.width / 2 - width / 2
        y: parent.height - height - units.dp(20)

        background: Rectangle {
            color: "#111111"
            opacity: 0.7
            radius: units.dp(10)
        }

        Text {
            id: popupLabel
            anchors.fill: parent
            horizontalAlignment: Text.AlignHCenter
            color: "#ffffff"
            font.pixelSize: units.dp(14)
        }

        Timer {
            id: popupTimer
            interval: 3000
            running: true
            onTriggered: {
                toast.close()
            }
        }

        function show(text) {
            popupLabel.text = text
            open()
            popupTimer.start()
        }
    }

    // Welcome to async hell:
    // 1. getGroups
    // 2. onGroupsReceived
    // 3. getEntries
    // 4. onEntriesReceived
    function populate() {
	if (groupUuid) {
            keepassrx.getGroups(groupUuid);
	} else {
	    keepassrx.getRootGroup();
	}
    }

    function getEntries(groupUuidToGet) {
        keepassrx.getEntries(groupUuidToGet, searchField.text);
    }

    Connections {
        target: keepassrx

        onDatabaseOpened: {
            populate();
        }

	// This is a list of groups underneath this group, not ALL the
	// groups. It's an array of RxListItem entities.
        onGroupsReceived: (parentGroupId, thisGroupId, thisGroupName, subgroups) => {
	    // Clear out searching when switching between groups.
	    searchMode = false;
	    searchField.text = '';

	    // the parent group id will be null if this is the root
	    parentGroupUuid = parentGroupId;
            previousGroupUuid = groupUuid;

            // Hack to let root group load once, but still be able to load sub-groups.
            if (!previousGroupUuid) previousGroupUuid = thisGroupId;

	    groupUuid = thisGroupId;
	    groupName = thisGroupName;
            getEntries(thisGroupId);
        }

	// List of entries for this group. It's an array of RxListItem
	// entities. It includes both immediate subgroups and
	// immediate child entries in the group.
        onEntriesReceived: (entries) => {
	    entriesListModel.clear();

            for (const entry of entries) {
                entriesListModel.append(entry);
            }
        }
    }
}
