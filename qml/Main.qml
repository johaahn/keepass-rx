

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
import Lomiri.Components 1.3
import KeepassRx 1.0

import "./pages"

ApplicationWindow {
    id: root
    objectName: 'mainView'
    visible: true
    width: units.gu(45)
    height: units.gu(75)

    Connections {
	target: keepassrx
	onDatabaseOpened: {
	    //busy = false;
	    root.push_entries(); // root = application window
	}

	onDatabaseOpenFailure: (error) => {
	    console.error(error);
	}
    }

    OpenDBPage {
        visible: false
        id: opendbPage
    }

    PageStack {
        id: stack
    }

    SettingsPage {
        visible: false
        id: settingsPage
    }

    function push_entries() {
        stack.push(Qt.resolvedUrl("pages/EntriesPage.qml"))
    }
    Component.onCompleted: stack.push(opendbPage)
}
