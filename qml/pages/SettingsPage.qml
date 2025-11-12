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
                    'Set header color and name according to public database settings.'
                )
                control: Switch {
                    onCheckedChanged: settings.showAccents = checked
                    checked: settings.showAccents
                }
            }

            SettingsItem {
                // TRANSLATORS: DB is the abbreviation for database
                title: i18n.tr("Auto-close database after inactivity")
                // TRANSLATORS: Description of the auto-close setting.
                description: i18n.tr("In minutes. 0 for disabled.")
                enabled: false
                control: TextField {
                    inputMethodHints: Qt.ImhDigitsOnly
                    text: settings.autoCloseInterval
                    onTextChanged: {
                        if (isNaN(parseInt(text))) {
                            text = 1
                        }
                        if (parseInt(text) < 0) {
                            text = 1
                        }
                        if (parseInt(text) > 100) {
                            text = 100
                        }

                        settings.autoCloseInterval = parseInt(text)
                    }
                    hasClearButton: false
                    validator: IntValidator {
                        bottom: 0
                        top: 100
                    }
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
