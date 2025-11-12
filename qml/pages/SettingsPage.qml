import QtQuick 2.0
import Lomiri.Components 1.3
import "../components"
import Qt.labs.settings 1.0

Page {
    property bool isARMv7: false
    header: PageHeader {
        id: header
        // TRANSLATORS: Page header for the settings.
        title: i18n.tr("Settings")
    }

    Flickable {
        id: flick
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.topMargin: units.gu(2)
        contentHeight: col.height
        contentWidth: width

        Column {
            Settings {
                id: settings
                property int autoCloseInterval: 5
                property bool showRecycleBin: false
                property bool changeGroupOnSearch: true
                property bool showSlowDBWarning: true
                property bool databaseLocking: true
                property bool showAccents: true
            }
            id: col
            anchors.fill: parent
            spacing: units.gu(2)

            anchors.leftMargin: units.gu(1)
            anchors.rightMargin: units.gu(1)

            Label {
                width: parent.width
                wrapMode: Text.WordWrap
                text: i18n.tr('Change settings based on your preferences. ' +
                              'Any change to the settings requires a restart to ' +
                              'take effect.')
            }

            // TODO Currently requires a restart because we have
            // several Settings instances floating around. Make them
            // into a singleton!
            SettingsItem {
                // TRANSLATORS: Enable or disable easy (and secure!) locking and unlocking of the database.
                title: i18n.tr("Enable database locking")
                // TRANSLATORS: Description of the database locking setting.
                description: i18n.tr(
                    "Securely lock and unlock database with a short passcode. Requires restart to take effect."
                )
                control: Switch {
                    onCheckedChanged: settings.databaseLocking = checked
                    checked: settings.databaseLocking
                }
            }

            SettingsItem {
                title: i18n.tr('Enable Accents')
                description: i18n.tr(
                    'Set header color and name according to database settings. ' +
                        'Compatible with KeePassXC.'
                )
                control: Switch {
                    onCheckedChanged: settings.showAccents = checked
                    checked: settings.showAccents
                }
            }

            SettingsItem {
                // TRANSLATORS: Whether or not to show the recycling bin group of deleted password entries.
                title: i18n.tr('Show the "Recycle bin" group')
                // TRANSLATORS: Description of the "Show recycle bin group" setting.
                description: i18n.tr('This group contains all the deleted entries')
                control: Switch {
                    onCheckedChanged: settings.showRecycleBin = checked
                    checked: settings.showRecycleBin
                }
            }
        }
    }
}
