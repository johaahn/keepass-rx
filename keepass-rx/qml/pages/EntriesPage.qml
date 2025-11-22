import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.3
import Lomiri.Components 1.3
import Lomiri.Components.Popups 1.3
import Lomiri.Content 1.3
import Qt.labs.settings 1.0
import keepassrx 1.0

import "../components"

Page {
    Settings {
        id: settings
        property bool showAccents: true
        property bool fetchOnOpen: false
        property bool showRecycleBin: false
        property bool databaseLocking: true
    }

    property bool searchMode: false
    property bool resetListView: false

    // These are set by metadata fetching
    property string publicDatabaseName
    property string recycleBinUuid

    property var colorWashout

    function lockDatabase() {
        keepassrx.closeDatabase();
        root.lockUI();
    }

    function closeDatabase() {
        keepassrx.invalidateMasterPassword();
        keepassrx.closeDatabase();
        root.closeUI();
    }

    function headerTitle() {
        if (containerStack.containerName && !containerStack.isAtRoot || keepassrx.viewMode != 'All') {
            return containerStack.containerName;
        } else {
            return settings.showAccents && publicDatabaseName ? publicDatabaseName : "KeePassRX";
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

    RxUiContainerStack {
        id: containerStack
        app: AppState
        viewMode: keepassrx.viewMode

        onViewModeChanged: (mode) => {
            entriesListModel.clear();
            resetListView = true;
        }

        onContainerChanged: (newContainerId) => {
            searchMode = false;
	    searchField.text = '';
            entriesListModel.clear();
            getEntries(newContainerId);
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
                    getEntries(containerStack.containerUuid);
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
                enabled: settings.databaseLocking && containerStack.isAtRoot
                visible: settings.databaseLocking && containerStack.isAtRoot
                name: "Lock"
                //TRANSLATORS: Securely lock (NOT close) an open database.
                text: i18n.tr("Lock")
                iconName: "lock"
                onTriggered: {
                    lockDatabase();
                }
            },

            Action {
                enabled: !settings.databaseLocking && containerStack.isAtRoot
                visible: !settings.databaseLocking && containerStack.isAtRoot
                name: "Close"
                //TRANSLATORS: Securely close (NOT lock) an open database.
                text: i18n.tr("Close")
                iconName: "close"
                onTriggered: {
                    closeDatabase();
                }
            },

	    Action {
                enabled: !containerStack.isAtRoot
                visible: !containerStack.isAtRoot
                name: "Go Back"
                //TRANSLATORS: Move back up in the container folder structure.
                text: i18n.tr("Back")
                iconName: "back"
                onTriggered: {
                    containerStack.popContainer();
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
                    PopupUtils.open(viewModeDialog);
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

    Item {
        id: changeViewMode

        Component {
            id: viewModeDialog

            Dialog {
                id: viewModeDialogInner
                // TRANSLATORS: Change list of entries that are shown (all, templated, 2fa, etc).
                title: i18n.tr("Change View")

                ListModel {
                    id: viewModes
                    ListElement {
                        name: "All"
                        menuText: QT_TR_NOOP("All Entries")
                        // TRANSLATORS: Keep sentence as short as possible.
                        description: QT_TR_NOOP("All groups and entries.")
                    }
                    ListElement {
                        name: "Templates"
                        menuText: QT_TR_NOOP("Special Categories")
                        // TRANSLATORS: Keep sentence as short as possible.
                        description: QT_TR_NOOP("Entries grouped by template.")
                    }
                    ListElement {
                        name: "Totp";
                        menuText: QT_TR_NOOP("2FA Codes")
                        // TRANSLATORS: Two-factor/OTP codes. Keep sentence as short as possible.
                        description: QT_TR_NOOP("Entries with 2FA codes.")
                    }
                    ListElement {
                        name: "Tags";
                        menuText: QT_TR_NOOP("Tags")
                        // TRANSLATORS: Lists of tagged entries. Keep sentence as short as possible.
                        description: QT_TR_NOOP("Entries grouped by tag.")
                    }
                }

                Component {
                    id: viewModeDelegate
                    OptionSelectorDelegate { text: i18n.tr(menuText); subText: i18n.tr(description) }
                }

                OptionSelector {
                    id: viewModeSelector
                    // TRANSLATORS: Select the list of entries that are shown
                    text: i18n.tr("Select View")
                    expanded: true
                    model: viewModes
                    delegate: viewModeDelegate
                    selectedIndex: getSelectedView()
                }

                function getSelectedView() {
                    for (let c = 0; c < viewModes.count; c++) {
                        const item = viewModes.get(c);
                        if (item.name == keepassrx.viewMode) {
                            return c;
                        }
                    }

                    return 0; // All entries (default)
                }

                Button {
                    text: i18n.tr("Select")
                    color: LomiriColors.green
                    onClicked: {
                        const selection = viewModes.get(viewModeSelector.selectedIndex);
                        keepassrx.viewMode = selection.name;
                        PopupUtils.close(viewModeDialogInner)
                    }
                }

                Button {
                    text: i18n.tr("Cancel")
                    color: LomiriColors.silk
                    onClicked: {
                        PopupUtils.close(viewModeDialogInner)
                    }
                }
            }
        }
    }

    Row {
        id: containerInstructionsLabel
        height: containerInstructionsText.height + containerInstructionsBottom.height
        anchors.top: parent.header.bottom
        width: parent.width
        visible: containerStack.instructions != null && containerStack.instructions.length > 0

        Column {
            width: parent.width
            Layout.fillWidth: true

            Text {
                id: containerInstructionsText
                text: containerStack.instructions
                color: theme.palette.normal.backgroundSecondaryText
                padding: units.gu(0.5)
                width: parent.width
                Layout.fillWidth: true
                wrapMode: Text.Wrap
            }

            Rectangle {
                id: containerInstructionsBottom
                Layout.fillWidth: true
                width: parent.width
                height: 1
                color: LomiriColors.orange
            }
        }
    }

    ListView {
	id: entriesList
        clip: true
        z: 1
        anchors.top: containerInstructionsLabel.visible ? containerInstructionsLabel.bottom : parent.header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        spacing: units.gu(0.1)

        model: ListModel {
            id: entriesListModel
        }

        delegate: EntryItem {
          uuid: entryUuid
        }
    }

    // Welcome to async hell:
    // 1. getContainers
    // 2. onContainersReceived
    // 3. getEntries
    // 4. onEntriesReceived
    function populate() {
	if (containerStack.isAtRoot) {
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

	// List of entries for this container. It's an array of uuids.
	// It includes both immediate subgroupings and immediate child
	// entries in the container.
        function onEntriesReceived(entries) {
          //Entries is a list of UUIDs
	    entriesListModel.clear();

            for (const entry of entries) {
                let append = true;
                if (entry.itemType == 'Group' && entry.uuid == recycleBinUuid) {
                    append = settings.showRecycleBin;
                }

                if (append) {
                    entriesListModel.append({entryUuid: entry });
                }
            }

            if (resetListView) {
                entriesList.positionViewAtBeginning();
                resetListView = false;
            }
        }
    }
}
