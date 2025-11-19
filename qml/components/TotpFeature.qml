import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3
import QtGraphicalEffects 1.0


Item {
    property string entryId

    width: parent.width
    height: parent.height

    Connections {
        target: keepassrx

        function onFieldValueReceived(entryUuid, fieldName, totpCode, totpValidFor) {
            // Because this is a global listener, we only want to update if this item is the
            // target for this update. Otherwise, all the TOTP codes are the same value!
            if (entryId == entryUuid && totpCode && totpValidFor) {
                current2FACode.text = totpCode;
                current2FAValidFor.text = totpValidFor;
                return;
            }
        }
    }

    Timer {
        id: current2FATimer
        repeat: true
        interval: 1000
        running: true
        triggeredOnStart: true
        onTriggered: {
            if (entryId) {
                keepassrx.getFieldValue(entryId, "CurrentTOTP");
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
            text: "------"
        }

        Text {
            id: current2FAValidFor
            elide: Text.ElideRight
            anchors.top: current2FACode.bottom
            height: parent.height / 2
            width: parent.width
            verticalAlignment: Text.AlignVCenter
            color: theme.palette.normal.backgroundTertiaryText
            text: "------"
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
