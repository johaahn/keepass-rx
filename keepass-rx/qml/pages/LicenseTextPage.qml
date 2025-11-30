import QtQuick 2.12
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3
import Qt.labs.settings 1.0
import keepassrx 1.0

Page {
    id: licenseTextPage
    property string projectName
    property string licenseText
    header: PageHeader {
        id: header
        title: projectName
    }

    ScrollView {
        width: licenseTextPage.width
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: units.gu(1.0)
        anchors.rightMargin: units.gu(1.0)

        ColumnLayout {
            Layout.fillWidth: true
            width: licenseTextPage.width - units.gu(2.0) // handle scrollview margins

            Label {
                Layout.fillWidth: true
                Layout.preferredWidth: licenseTextPage.width
                text: licenseText
                wrapMode: Text.Wrap
            }
        }
    }
}
