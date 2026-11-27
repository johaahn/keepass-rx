import QtQuick 2.6
import Sailfish.Silica 1.0

Rectangle {
    id: savingOverlay
    objectName: "savingOverlay"
    color: Theme.rgba(Theme.overlayBackgroundColor, Theme.opacityOverlay)
    visible: opacity > 0
    opacity: keepassrx.saving ? 1 : 0

    Behavior on opacity { FadeAnimation {} }

    MouseArea {
        anchors.fill: parent
    }

    Column {
        anchors.centerIn: parent
        width: parent.width - 2 * Theme.horizontalPageMargin
        spacing: Theme.paddingLarge

        BusyIndicator {
            anchors.horizontalCenter: parent.horizontalCenter
            size: BusyIndicatorSize.Large
            running: savingOverlay.visible
        }

        Label {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: Tr.tr("Saving...")
            color: Theme.highlightColor
        }

        Label {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            text: Tr.tr("Do not close the app.")
            color: Theme.secondaryHighlightColor
            font.pixelSize: Theme.fontSizeSmall
        }
    }
}
