import QtQuick 2.12
import QtQuick.Layouts 1.12
import QtQuick.Controls 2.12
import Qt.labs.settings 1.0
import Lomiri.Components 1.3
import Lomiri.Components.Popups 1.3
import Lomiri.Content 1.3
import KeepassRx 1.0
import "../components"

Page {
    id: dbListPage

    property bool copyingDB
    property bool pickingDB
    property bool busy
    property string errorMsg

    Settings {
        id: settings
        property string lastKey
        property string lastDB
        property int autoCloseInterval: 5
        property bool showSlowDBWarning: true
        property bool easyOpen: true
    }

    Connections {
        target: keepassrx

        onDatabaseImported: (databaseName) => {
            dbListModel.append({ databaseName });
        }

        onDatabaseDeleted: (databaseName) => {
            let indexToDelete = undefined;
            for (let c = 0; c < dbListModel.count; c++) {
                const entry = dbListModel.get(c);
                if (entry.databaseName === databaseName) {
                    indexToDelete = c;
                    break;
                }
            }

            if (indexToDelete !== undefined) {
                dbListModel.remove(indexToDelete);
            }
        }

        onFileListingCompleted: {
            lastDbTimer.running = true;
        }
    }

    // Immediately go to the enter pw page if we have selected a
    // DB in the past.
    // TODO check if file actually exists!
    Timer {
        id: lastDbTimer
        interval: 1
        running: false
        repeat: false
        onTriggered: {
            if (keepassrx.lastDB) {
                let openPage = Qt.resolvedUrl("./OpenDBPage.qml");
                pageStack.addPageToCurrentColumn(
                    dbListPage, openPage, {
                        databaseName: keepassrx.lastDB
                    }
                );
            }
        }
    }

    Component.onCompleted: {
        if (keepassrx.databaseOpen) {
            console.log('OpenDBPage: Closing an already open database. This is an anomaly.');
            keepassrx.closeDatabase();
        }

        keepassrx.listImportedDatabases();
    }

    header: PageHeader {
        id: header
        title: "KeePassRX"
        trailingActionBar.numberOfSlots: 2
        trailingActionBar.actions: [
            Action {
                name: "Add Database"
                text: i18n.tr("Add Database")
                iconName: "add"
                onTriggered: { peerPicker.visible = true; }
            },

            Action {
                name: "Settings"
                text: i18n.tr("Settings")
                iconName: "settings"
                onTriggered: { pageStack.addPageToNextColumn(dbListPage, settingsPage) }
            },

            Action {
                name: "About"
                text: i18n.tr("About")
                iconName: "info"
                onTriggered: { pageStack.addPageToNextColumn(dbListPage, aboutPage) }
            }
        ]
    }

    ColumnLayout {
        id: loadLastDbView
        visible: keepassrx.lastDB && keepassrx.lastDB.length > 0 // string
        spacing: units.gu(2)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        Layout.fillWidth: true
        Layout.fillHeight: true
        height: parent.height
        width: parent.width

        Text {
            text: i18n.tr("Loading last database:", keepassrx.lastDB.length);
        }

    }


    ContentPeerPicker {
        id: peerPicker
        visible: false
        showTitle: true
        //TRANSLATORS: The user is adding a KeePass database to the list of DBs in the app.
        headerText: i18n.tr("Select Database")
        z: 10 // make sure to show above everything else.
        handler: ContentHandler.Source
        contentType: ContentType.All

        // Picker is closed by signalConnections after DB copied.
        onPeerSelected: {
            peer.selectionType = ContentTransfer.Single;
            copyDatabase.target = peer.request();
        }

        onCancelPressed: peerPicker.visible = false;
    }

    Connections {
        id: copyDatabase
        onStateChanged: {
            var done = target.state === ContentTransfer.Charged;

            if (!done) {
                return;
            }
            if (target.items.length === 0) {
                return;
            }

            const filePath = String(target.items[0].url).replace('file://', '');
            copyingDB = true;
            keepassrx.importDatabase(filePath, pickingDB);
            target.finalize();
            peerPicker.visible = false;
        }
    }

    // Initial logo image.
    ColumnLayout {
        visible: dbListModel.count == 0 && !keepassrx.lastDB
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        anchors.leftMargin: units.gu(7)
        anchors.rightMargin: units.gu(7)
        anchors.verticalCenter: parent.verticalCenter
        spacing: units.gu(2)

        RowLayout {
            Layout.fillWidth: true

            Rectangle {
                height: units.gu(35)
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter | Qt.AlignHCenter
                color: "transparent"

                Text {
                    color: LomiriColors.slate
                    text: i18n.tr("Tap + to import a database.")
                    horizontalAlignment: Qt.AlignHCenter
                    x: parent.width / 2 - width / 2
                }

                Image {
                    id: logo
                    width: units.gu(20)
                    height: units.gu(20)
                    fillMode: Image.PreserveAspectFit
                    source: '../../assets/keepass-rx.svg'
                    x: parent.width / 2 - width / 2
                    y: parent.height / 2 - height / 2
                }
            }
        }
    }

    // DB list
    ColumnLayout {
        visible: dbListModel.count > 0 && !keepassrx.lastDB
        spacing: units.gu(2)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        Layout.fillWidth: true
        Layout.fillHeight: true
        height: parent.height
        width: parent.width

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            height: parent.height
            width: parent.width

            LomiriListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                height: parent.height
                width: parent.width

                id: dbList
                model: ListModel {
                    id: dbListModel
                }

                delegate: ListItem {
                    height: layout.height + (divider.visible ? divider.height : 0)

                    ListItemLayout {
                        id: layout
                        title.text: databaseName
                        // TODO get subtitle text and good vertical alignment.

                        Icon {
                            name: "next"
                            SlotsLayout.position: SlotsLayout.Trailing;
                            height: units.gu(2)
                        }
                    }

                    onClicked: {
                        keepassrx.lastDB = databaseName;

                        pageStack.addPageToCurrentColumn(
                            dbListPage, Qt.resolvedUrl("./OpenDBPage.qml"), {
                                databaseName
                            }
                        );
                    }

                    leadingActions: ListItemActions {
                        actions: [
                            Action {
                                iconName: "delete"
                                onTriggered: {
                                    deleteDatabase.databaseName = databaseName;
                                    PopupUtils.open(dialog)
                                }
                            }
                        ]
                    }
                }
            }
        }
    }

    Item {
        id: deleteDatabase
        width: units.gu(80)
        height: units.gu(80)
        property string databaseName

        Component {
            id: dialog

            Dialog {
                id: dialogue
                title: "Delete Database"
                text: i18n.tr(
                    `Are you sure you want to remove ${deleteDatabase.databaseName}? ` +
                        'It will only be removed from the app, not your device.')
                Button {
                    text: "Cancel"
                    onClicked: PopupUtils.close(dialogue)
                }
                Button {
                    text: "Remove"
                    color: LomiriColors.red
                    onClicked: {
                        keepassrx.deleteDatabase(deleteDatabase.databaseName);
                        deleteDatabase.databaseName = null;
                        PopupUtils.close(dialogue)
                    }
                }
            }
        }
    }
}
