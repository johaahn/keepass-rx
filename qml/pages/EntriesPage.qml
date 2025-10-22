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
        property bool tapToReveal: true
        property bool showRecycleBin: false
        property bool changeGroupOnSearch: true
    }

    property bool searchMode: false

    id: sectionFlickable

    header: UITK.PageHeader {
        id: header
	title: "KeepassRX"
	trailingActionBar.numberOfSlots: 3
	trailingActionBar.actions: [
	    UITK.Action {
		name: "Settings"
		text: i18n.tr("Settings")
		iconName: "settings"
		onTriggered: { stack.push(settingsPage) }
	    },

	    UITK.Action {
		name: "About"
		text: i18n.tr("About")
		iconName: "info"
		onTriggered: { stack.push(aboutPage) }
	    },

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
	    }
	]
    }

    ColumnLayout {
	id: opsBar
	Layout.fillWidth: true

	anchors {
	    margins: units.gu(1)
	    top: header.bottom
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
		placeholderText: i18n.ctr("text for search placeholder", "Search")
		inputMethodHints: Qt.ImhNoPredictiveText
		onTextChanged: {
		    getEntries();
		}
	    }
	}

	RowLayout {
	    id: groupsBar
	    Layout.fillWidth: true
	    width: parent.width

	    UITK.Sections {
		id: sections
		width: parent.width
		Layout.fillWidth: true
		model: []
		onSelectedIndexChanged: {
		    getEntries();
		}
	    }
	}
    }


    ListView {
	clip: true
	z: 1
	anchors.top: opsBar.bottom
	anchors.left: parent.left
	anchors.right: parent.right
	anchors.bottom: parent.bottom
	spacing: units.gu(0.1)

	id: lv
	model: ListModel {
            id: listmodel
	}

	delegate: DBEntry {}
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
	keepassrx.getGroups();
    }

    function getEntries() {
	const group = sections.model[sections.selectedIndex]
	keepassrx.getEntries(searchField.text);
    }

    Connections {
	target: keepassrx

	onGroupsReceived: (groups) => {
	    sections.model = groups;
	    getEntries();
	}

	onEntriesReceived: (items) => {
	    const group = sections.model[sections.selectedIndex];
	    listmodel.clear();
            let entries = items[group] || [];

            if (settings.changeGroupOnSearch && !entries.length) {
		const keys = Object.keys(items);
		if (keys.length) {
                    sections.selectedIndex = sections.model.indexOf(keys[0])
                    return;
		}
            }

            for (var i = 0; i < entries.length; i++) {
		listmodel.append(entries[i]);
            }
	}
    }

    Component.onCompleted: populate()
}
