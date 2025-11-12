import QtQuick 2.0
import Lomiri.Components 1.3 as UITK
import "../components"
import Qt.labs.settings 1.0

UITK.Page {
    property bool isARMv7: false
    header: UITK.PageHeader {
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
                title: i18n.tr("Enable database locking (ALPHA)")
                // TRANSLATORS: Description of the database locking setting.
                description: i18n.tr(
                    "Securely lock and unlock database with a short passcode. Requires restart to take effect."
                )
                control: UITK.Switch {
                    onCheckedChanged: settings.databaseLocking = checked
                    checked: settings.databaseLocking
                }
            }

            SettingsItem {
                title: i18n.tr('Enable Accents')
                description: i18n.tr(
                    'Set header color and name according to public database settings.'
                )
                control: UITK.Switch {
                    onCheckedChanged: settings.showAccents = checked
                    checked: settings.showAccents
                }
            }

            SettingsItem {
                // TRANSLATORS: DB is the abbreviation for database
                title: i18n.tr("Auto-close db after inactivity")
                // TRANSLATORS: Description of the auto-close setting.
                description: i18n.tr("In minutes. 0 for disabled.")
                control: UITK.TextField {
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
                control: UITK.Switch {
                    onCheckedChanged: settings.showRecycleBin = checked
                    checked: settings.showRecycleBin
                }
            }
            SettingsItem {
                // TRANSLAOTORS: Whether or not to change group section on search.
                title: i18n.tr('Change section on search')
                description: i18n.tr(
                    'Change section automatically if there are no results for the ' +
                        'search value in the current section, and there are results in another section'
                )
                control: UITK.Switch {
                    onCheckedChanged: settings.changeGroupOnSearch = checked
                    checked: settings.changeGroupOnSearch
                }
            }
            SettingsItem {
                visible: isARMv7
                title: i18n.tr('Show warning before opening very slow databases')
                description: i18n.tr(
                    'Opening KDBX3 databases on ARMv7 devices can take up to 2 seconds <b>per entry</b> (3 minutes for 100 entries)'
                )
                control: UITK.Switch {
                    onCheckedChanged: settings.showSlowDBWarning = checked
                    checked: settings.showSlowDBWarning
                }
            }
        }
    }
}
