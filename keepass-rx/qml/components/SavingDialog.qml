import QtQuick 2.12
import Lomiri.Components 1.3
import Lomiri.Components.Popups 1.3

Dialog {
    id: savingDialogue

    Column {
        width: parent.width
        spacing: units.gu(1)

        ActivityIndicator {
            anchors.horizontalCenter: parent.horizontalCenter
            running: true
        }

        Label {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            textSize: Label.Large
            text: i18n.tr("Saving...")
        }

        Label {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            color: theme.palette.normal.backgroundSecondaryText
            text: i18n.tr("Do not close the app.")
        }
    }
}
