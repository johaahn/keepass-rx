import QtQuick 2.12
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3
import "../components"
import Qt.labs.settings 1.0

Page {
    id: aboutPage
    header: PageHeader {
        id: header
        // TRANSLATORS: Header for the "About" page.
        title: i18n.tr("About")

        trailingActionBar.numberOfSlots: 1
        trailingActionBar.actions: [
            Action {
                name: "Open Source Licenses"
                iconName: "stock_document"
                onTriggered: {
                    pageStack.addPageToNextColumn(
                        aboutPage,
                        Qt.resolvedUrl("qrc:/webengine/qml/pages/LicensesPage.qml")
                    );
                }
            }
        ]
    }

    ScrollView {
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        width: aboutPage.width
        anchors.leftMargin: units.gu(1.0)
        anchors.rightMargin: units.gu(1.0)

        ColumnLayout {
            width: aboutPage.width - units.gu(2.0) // handle scrollview margins
            spacing: units.gu(1.5)

            Label {
                id: appTitle
                Layout.fillWidth: true
                text: "KeePassRx"
                textSize: Label.XLarge
                horizontalAlignment: Text.AlignHCenter
            }

            RowLayout {
                Layout.fillWidth: true

                Rectangle {
                    height: units.gu(30)
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignVCenter | Qt.AlignHCenter
                    color: "transparent"

                    Image {
                        width: units.gu(30)
                        height: units.gu(30)
                        fillMode: Image.PreserveAspectFit
                        source: '../../assets/keepass-rx.svg'
                        x: parent.width / 2 - width / 2
                        y: parent.height / 2 - height / 2
                    }
                }
            }

            Item {
                Layout.fillHeight: false
                Layout.fillWidth: true
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: i18n.tr(
                    "" +
                        "KeePassRX is password manager for KeePass databases, " +
                        "designed for Ubuntu Touch. It is licensed under the " +
                        "GNU AGPL v3 license."
                )
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: i18n.tr(
                    "" +
                        "The built-in KeePass icon images are licensed under a variety of " +
                        "licenses as detailed in assets/COPYING."
                )
            }

            Label {
                Layout.fillWidth: true
                text: i18n.tr("Manual")
                textSize: Label.Large
                lineHeight: 0.5
            }

            Text {
                Layout.fillWidth: true
                color: theme.palette.normal.backgroundSecondaryText
                wrapMode: Text.Wrap
                lineHeight: 0.5
                leftPadding: units.gu(1.5)
                text: i18n.tr(
                    "• Gemini: <a href='gemini://agnos.is/projects/keepassrx/'>" +
                        "gemini://agnos.is/projects/keepassrx/</a>"
                )
                onLinkActivated: Qt.openUrlExternally(link)
            }

            Text {
                Layout.fillWidth: true
                color: theme.palette.normal.foregroundText
                wrapMode: Text.Wrap
                lineHeight: 0.5
                leftPadding: units.gu(1.5)
                text: i18n.tr(
                    "• Web: <a href='https://agnos.is/projects/keepassrx/'>" +
                        "https://agnos.is/projects/keepassrx/</a>"
                )
                onLinkActivated: Qt.openUrlExternally(link)
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: i18n.tr("Source Code")
                textSize: Label.Large
            }

            Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: theme.palette.normal.backgroundSecondaryText
                text: i18n.tr(
                    "<a href='https://git.agnos.is/projectmoon/keepass-rx'>" +
                        "https://git.agnos.is/projectmoon/keepass-rx</a>"
                )
                onLinkActivated: Qt.openUrlExternally(link)
            }
        }
    }
}
