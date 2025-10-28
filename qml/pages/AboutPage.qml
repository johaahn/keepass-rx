import QtQuick 2.0
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3 as UITK
import "../components"
import Qt.labs.settings 1.0

/* width: parent.width */
/*             Layout.preferredWidth: width // this plus the explicit width and wrapmode will wrap the long text. */
UITK.Page {
    property bool isARMv7: false
    header: UITK.PageHeader {
        id: header
        title: i18n.ctr("page header", "About")
    }

    ColumnLayout {
        spacing: units.gu(2)

        RowLayout {
            Layout.fillWidth: true

            Rectangle {
                height: units.gu(35)
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter | Qt.AlignHCenter
                color: "transparent"

                Image {
                    width: units.gu(35)
                    height: units.gu(35)
                    fillMode: Image.PreserveAspectFit
                    source: '../../assets/keepass-rx.svg'
                    x: parent.width / 2 - width / 2
                    y: parent.height / 2 - height / 2
                }
            }
        }

        anchors {
            margins: units.gu(2)
            top: header.bottom
            left: parent.left
            right: parent.right
            bottom: parent.bottom
        }

        Item {
            Layout.fillHeight: false
        }

        UITK.Label {
            width: parent.width
            Layout.preferredWidth: width
            wrapMode: Text.Wrap
            text: i18n.tr(
                "" +
                    "KeePassRX is a work-in-progress password manager for KeePass " +
                    "password databases. It is licensed under the GNU GPL v3 license."
            )
        }

        UITK.Label {
            width: parent.width
            Layout.preferredWidth: width
            wrapMode: Text.Wrap
            text: i18n.tr(
                "Source: <a href='https://git.agnos.is/projectmoon/keepass-rx'>" +
                    "https://git.agnos.is/projectmoon/keepass-rx</a>"
            )
            onLinkActivated: Qt.openUrlExternally(link)
        }
        Item {
            Layout.fillHeight: true
        }
    }
}
