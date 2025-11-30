import QtQuick 2.12
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3
import Qt.labs.settings 1.0
import keepassrx 1.0

Page {
    id: licensesPage

    header: PageHeader {
        id: header
        // TRANSLATORS: List of 3rd party licenses used in this project.
        title: i18n.tr("Open Source Licenses")
    }

    RxUiLicenses {
        id: uiLicenses
    }

    ListView {
	id: licenseList
        clip: true
        z: 1
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom

        model: ListModel {
            id: licenseListModel
        }

        delegate: ListItem {
            height: layout.height + (divider.visible ? divider.height : 0)
            width: licenseList.width

            ListItemLayout {
                id: layout
                title.text: `${crateName} ${crateVersion}`
                subtitle.text: licenseName

                Icon {
                    name: "next"
                    SlotsLayout.overrideVerticalPositioning: true
                    SlotsLayout.position: SlotsLayout.Trailing
                    width: units.gu(3)
                    height: units.gu(3)
                    y: layout.subtitle.y - baselineOffset
                }
            }

            onClicked: {
                pageStack.addPageToCurrentColumn(
                    licensesPage,
                    Qt.resolvedUrl("qrc:/qml/pages/LicenseTextPage.qml"),
                    {
                        projectName: `${crateName} ${crateVersion}`,
                        licenseText: licenseText
                    }
                );
            }

            trailingActions: ListItemActions {
                actions: [
                    Action {
                        name: i18n.tr('Website')
                        iconName: "external-link"
                        onTriggered: {
                            Qt.openUrlExternally(crateURL);
                        }
                    }
                ]
            }
        }

        Component.onCompleted: {
            for (const license of uiLicenses.allLicenses()) {
                licenseListModel.append(license);
            }
        }
    }
}
