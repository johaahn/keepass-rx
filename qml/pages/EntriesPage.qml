import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.3
import Lomiri.Components 1.3
import Qt.labs.settings 1.0

import "../components"

Page {
    Settings {
        id: settings
        property bool showAccents: true
        property bool fetchOnOpen: false
        property bool showRecycleBin: false
        property bool databaseLocking: true
    }

    // The default values of these properties control what we should fetch first.
    property string containerUuid
    property string containerName
    property string containerType: 'Group'
    property bool atRoot: true
    property bool searchMode: false

    // These are set by metadata fetching
    property string publicDatabaseName
    property string recycleBinUuid

    property var colorWashout

    onContainerUuidChanged: {
	populate();
    }

    function lockDatabase() {
        keepassrx.closeDatabase();
        containerUuid = null;
        containerName = null;
        root.lockUI();
    }

    function closeDatabase() {
        keepassrx.invalidateMasterPassword();
        keepassrx.closeDatabase();
        containerUuid = null;
        containerName = null;
        root.closeUI();
    }

    function isAtRoot() {
        return atRoot;
    }

    function headerTitle() {
        if (containerName && !isAtRoot()) {
            return containerName;
        } else {
            if (containerType == 'Group') {
                return settings.showAccents && publicDatabaseName ? publicDatabaseName : "KeePassRX";
            } else {
                return i18n.tr("Special Categories");
            }
        }
    }

    function headerBackgroundColor() {
        if (settings.showAccents && colorWashout) {
            return colorWashout.backgroundColor;
        } else {
            return "transparent";
        }
    }

    function headerTextColor() {
        if (settings.showAccents && colorWashout) {
            // textColorType is the color type for the header text itself.
            return colorWashout.textColorType === 'Light'
                ? LomiriColors.white
                : LomiriColors.jet;
        } else {
            return theme.palette.normal.foregroundText;
        }
    }

    PageHeader {
        id: opsBar
        visible: searchMode
        Layout.fillWidth: true

        StyleHints {
            backgroundColor: headerBackgroundColor()
            foregroundColor: headerTextColor()
        }

        leadingActionBar.actions: [
	    Action {
                name: "Cancel Search"
                //TRANSLATORS: End the search operation.
                text: i18n.tr("Cancel Search")
                iconName: "back"
                onTriggered: {
                    searchMode = false;
                    searchField.text = '';
                }
            }
        ]

        contents: RowLayout {
            id: searchBar
            Layout.fillWidth: true
            width: parent.width
            height: parent.height
            TextField {
                width: parent.width
                Layout.fillWidth: true
                visible: searchMode
                id: searchField
                // TRANSLATORS: Placeholder text of the search box for searching for database entries. Container is a container/folder of password manager entries.
                placeholderText: i18n.tr("Search entries in this group")
                inputMethodHints: Qt.ImhNoPredictiveText
                onTextChanged: {
                    getEntries(containerUuid);
                }
            }
        }
    }

    PageHeader {
        id: regularHeader
        visible: !searchMode
        title: headerTitle()

        StyleHints {
            backgroundColor: headerBackgroundColor()
            foregroundColor: headerTextColor()
        }

        leadingActionBar.actions: [
            Action {
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

            Action {
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

	    Action {
                enabled: !isAtRoot()
                visible: !isAtRoot()
                name: "Go Back"
                //TRANSLATORS: Move back up in the container folder structure.
                text: i18n.tr("Back")
                iconName: "back"
                onTriggered: {
                    keepassrx.popContainer();
                }
            }
        ]

        trailingActionBar.numberOfSlots: 3
        trailingActionBar.actions: [
            Action {
                name: "View Mode"
                // TRANSLATORS: Method of showing entries: by container or by template type.
                text: i18n.tr("View Mode")
                iconName: "filters"
                onTriggered: {
                    if (keepassrx.viewMode == 'Templates') {
                        keepassrx.viewMode = 'All';
                    } else {
                        keepassrx.viewMode = 'Templates';
                    }
                }
            },

            Action {
                name: "Search"
                // TRANSLATORS: Initiate (or stop) the search action.
                text: i18n.tr("Search")
                iconName: "search"
                onTriggered: {
                    searchField.text = '';
                    searchMode = !searchMode;
                    if (searchMode) {
                        searchField.focus = true;
                    }
                }
            },

            Action {
                name: "Settings"
                text: i18n.tr("Settings")
                iconName: "settings"
                onTriggered: {
                    pageStack.addPageToNextColumn(adaptiveLayout.primaryPage, settingsPage)
                }
            },

            Action {
                name: "About"
                text: i18n.tr("About")
                iconName: "info"
                onTriggered: {
                    pageStack.addPageToNextColumn(adaptiveLayout.primaryPage, aboutPage)
                }
            },

            Action {
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

            Action {
                name: "Close"
                // TRANSLATORS: Close (NOT lock) an open database.
                text: i18n.tr('Close Database')
                iconName: "close"
                onTriggered: {
                    closeDatabase();
                }
            }
        ]
    }

    header: searchMode ? opsBar : regularHeader

    ListView {
	id: entriesList
        clip: true
        z: 1
        anchors.top: parent.header.bottom
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
    // 1. getContainers
    // 2. onContainersReceived
    // 3. getEntries
    // 4. onEntriesReceived
    function populate() {
	if (isAtRoot()) {
            keepassrx.getRootContainer();
	} else {
            keepassrx.getContainer(containerUuid);
	}
    }

    function getEntries(containerUuidToGet) {
        keepassrx.getEntries(containerUuidToGet, searchField.text);
    }

    Connections {
        target: keepassrx

        function onDatabaseOpened() {
            // This will trigger the cascade of async operations that
            // will fetch entries.
            keepassrx.viewMode = 'All';
            keepassrx.getMetadata();

            const metadata = keepassrx.metadata;

            if (metadata.publicName) {
                publicDatabaseName = metadata.publicName;
            }

            if (metadata.publicColor) {
                colorWashout = keepassrx.washOutColor(metadata.publicColor);
            }

            if (metadata.recycleBinUuid) {
                recycleBinUuid = metadata.recycleBinUuid;
            }
        }

        function onViewModeChanged(mode) {
            entriesListModel.clear();
        }

        // newContainer is { containerUuid, containerType }
        function onCurrentContainerChanged(newContainer) {
	    searchMode = false;
	    searchField.text = '';
            entriesListModel.clear();

            // Type must be set before UUID due to uuid change signal
            // triggering entry list update.
            atRoot = newContainer.isRoot;
            containerType = newContainer.containerType;
            containerUuid = newContainer.containerUuid;
        }

        // Put as list of folders. When tapped, load template entries
        // and onEntriesReceived takes care of the rest? BUT... we
        // also have to take into account the container UUIDs.
        function onContainerReceived(thisContainerId, thisContainerName) {
            searchMode = false;
	    searchField.text = '';
            entriesListModel.clear();

	    containerName = thisContainerName;
            getEntries(thisContainerId);
        }

	// List of entries for this container. It's an array of
	// RxListItem entities. It includes both immediate subgroups
	// and immediate child entries in the container.
        function onEntriesReceived(entries) {
	    entriesListModel.clear();

            for (const entry of entries) {
                let append = true;
                if (entry.itemType == 'Group' && entry.uuid == recycleBinUuid) {
                    append = settings.showRecycleBin;
                }

                if (append) {
                    entriesListModel.append(entry);
                }
            }
        }
    }
}
