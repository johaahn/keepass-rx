import QtQuick 2.6
import Sailfish.Silica 1.0

Dialog {
    id: dialog
    property bool rejected: false
    property bool fallback: false
    property string entryTitle
    property string password

    canAccept: passwordField.text.length > 0

    onAccepted: password = passwordField.text

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: column.height

        Column {
            id: column
            width: parent.width

            DialogHeader {
                acceptText: Tr.tr("Save")
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                visible: dialog.fallback
                wrapMode: Text.Wrap
                color: Theme.highlightColor
                text: Tr.tr("The fingerprint or security code check did not complete.")
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.Wrap
                color: Theme.highlightColor
                text: Tr.tr("Enter your password to save the changes to %1.").arg(dialog.entryTitle)
            }

            PasswordField {
                id: passwordField
                width: parent.width
                label: Tr.tr("Password")
                placeholderText: label
                errorHighlight: dialog.rejected
                description: dialog.rejected ? Tr.tr("Incorrect password. Try again.") : ""
                inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                focus: true
                EnterKey.enabled: text.length > 0
                EnterKey.iconSource: "image://theme/icon-m-enter-accept"
                EnterKey.onClicked: dialog.accept()
                onTextChanged: dialog.rejected = false
            }
        }
    }
}
