

/*
 * Copyright (C) 2021  David Ventura
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; version 3.
 *
 * Keepass is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */
import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.3
import Qt.labs.settings 1.0
import Lomiri.Components 1.3
import KeepassRx 1.0

import "./pages"

MainView {
    id: root
    objectName: 'mainView'
    anchorToKeyboard: true

    visible: true
    width: units.gu(45)
    height: units.gu(75)

    Settings {
        id: settings
        property string lastDB
    }

    Connections {
	target: keepassrx
        onErrorReceived: (error) => {
            console.error('Uncaught error (put me in a popup):', error);
        }
    }

    Component.onCompleted: {
        reload();
    }

    function lockUI() {
        if (adaptiveLayout.primaryPage) {
            adaptiveLayout.removePages(adaptiveLayout.primaryPage);
        }

        if (adaptiveLayout.primaryPageSource) {
            adaptiveLayout.removePages(adaptiveLayout.primaryPageSource);
        }

        adaptiveLayout.primaryPageSource = Qt.resolvedUrl("pages/UnlockPage.qml");
    }

    function closeUI() {
        if (adaptiveLayout.primaryPage) {
            adaptiveLayout.removePages(adaptiveLayout.primaryPage);
        }

        if (adaptiveLayout.primaryPageSource) {
            adaptiveLayout.removePages(adaptiveLayout.primaryPageSource);
        }

        adaptiveLayout.primaryPageSource = Qt.resolvedUrl("pages/DBList.qml");
    }

    // Only one of dblist or unlock page can be active at a time, due
    // to conflicting signals.
    function reload() {
        if (adaptiveLayout.primaryPage) {
            adaptiveLayout.removePages(adaptiveLayout.primaryPage);
        }

        if (adaptiveLayout.primaryPageSource) {
            adaptiveLayout.removePages(adaptiveLayout.primaryPageSource);
        }

        if (keepassrx.isMasterPasswordEncrypted) {
            adaptiveLayout.primaryPageSource = Qt.resolvedUrl("pages/UnlockPage.qml");
        } else {
            adaptiveLayout.primaryPageSource = Qt.resolvedUrl("pages/DBList.qml");
        }
    }

    AdaptivePageLayout {
	id: adaptiveLayout
	anchors.fill: parent

        OpenDBPage {
            id: openDbPage
            visible: false
        }

	EntriesPage {
	    visible: false
	    id: entriesPage
	}

	SettingsPage {
	    visible: false
	    id: settingsPage
	}

	AboutPage {
	    visible: false
	    id: aboutPage
	}
    }
}
