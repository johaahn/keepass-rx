import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3
import QtGraphicalEffects 1.0
import keepassrx 1.0

Item {
    property string entryId

    width: parent.width
    height: parent.height

    RxUiEntry {
        id: theEntry
        entryUuid: uuid
        app: AppState
    }

    Timer {
        id: current2FATimer
        repeat: true
        interval: 1000
        running: true
        triggeredOnStart: true
        onTriggered: {
            if (entryId) {
                theEntry.updateTotp();
            }
        }
    }

    Rectangle {
        id: featuresColumn
        visible: true
        color: "transparent"
        width: parent.width
        height: parent.height

        MouseArea {
            anchors.left: parent.left
            anchors.right: parent.right
            z: 10
            width: parent.height
            height: parent.width
            onClicked: {
                // TODO migrate to actorify?
                keepassrx.getTotp(entryId);
            }
        }

        Text {
            id: current2FACode
            elide: Text.ElideRight
            height: parent.height / 2
            width: parent.width
            verticalAlignment: Text.AlignVCenter
            color: theme.palette.normal.backgroundTertiaryText
            //text: "------"
            text: theEntry.currentTotp
        }

        Text {
            id: current2FAValidFor
            elide: Text.ElideRight
            anchors.top: current2FACode.bottom
            height: parent.height / 2
            width: parent.width
            verticalAlignment: Text.AlignVCenter
            color: theme.palette.normal.backgroundTertiaryText
            text: theEntry.currentTotpValidFor
        }

        Icon {
            id: clockIcon
            name: 'clock'
            width: units.gu(2)
            height: units.gu(2)

            anchors.bottom: current2FAValidFor.bottom
            anchors.right: current2FAValidFor.right
            anchors.rightMargin: clockIcon.width * 1.025
            anchors.verticalCenter: current2FAValidFor.verticalCenter
        }
    }
}
