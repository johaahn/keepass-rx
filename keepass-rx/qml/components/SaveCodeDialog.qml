import QtQuick 2.12
import Lomiri.Components 1.3
import Lomiri.Components.Popups 1.3

Dialog {
    id: codeDialogue
    property bool rejected: false
    property int triesLeft: 0
    property string entryTitle

    signal confirmed(string code)

    title: i18n.tr("Unlock to Save")
    text: i18n.tr("Enter your passcode to save the changes to %1.").arg(entryTitle) + "\n\n" + i18n.tr(
        "The passcode is the first five characters of the database " +
            "password (or the whole password if less than five characters)."
    )

    function confirm() {
        if (codeField.text.length === 0) {
            return;
        }

        const code = codeField.text;
        PopupUtils.close(codeDialogue);
        codeDialogue.confirmed(code);
    }

    TextField {
        id: codeField
        placeholderText: i18n.tr("Passcode")
        echoMode: TextInput.Password
        inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
        Keys.onReturnPressed: codeDialogue.confirm()
        Component.onCompleted: forceActiveFocus()
    }

    Label {
        visible: codeDialogue.rejected
        text: i18n.tr("Incorrect passcode. %1 try left.", "Incorrect passcode. %1 tries left.", codeDialogue.triesLeft).arg(codeDialogue.triesLeft)
        color: theme.palette.normal.negative
        wrapMode: Text.WordWrap
    }

    Button {
        text: i18n.tr("Save")
        color: theme.palette.normal.positive
        enabled: codeField.text.length > 0
        onClicked: codeDialogue.confirm()
    }

    Button {
        text: i18n.tr("Cancel")
        onClicked: PopupUtils.close(codeDialogue)
    }
}
