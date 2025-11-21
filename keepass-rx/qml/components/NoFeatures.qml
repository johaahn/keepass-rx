import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3
import QtGraphicalEffects 1.0

Item {
    width: parent.width
    height: parent.height

    Rectangle {
        color: "transparent"
        width: parent.width
        height: parent.height

        MouseArea {
            x: parent.x
            width: parent.width
            height: parent.height
            onClicked: handleEntryClick()
        }

        Icon {
            width: units.gu(2.8)
            height: units.gu(2.8)
            color: theme.palette.normal.foregroundText
            x: parent.x + width / 1.25
            y: parent.height / 2 - height / 2
            name: 'next'
        }
    }
}
