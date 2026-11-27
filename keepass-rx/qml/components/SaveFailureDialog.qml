import QtQuick 2.12
import Lomiri.Components 1.3
import Lomiri.Components.Popups 1.3

Dialog {
    id: failureDialogue
    property string message
    property bool conflict

    title: i18n.tr("Could not save entry")
    text: conflict
        ? i18n.tr("The database file changed on disk. Close and reopen the database, then edit again.")
        : message

    Button {
        text: i18n.tr("OK")
        onClicked: PopupUtils.close(failureDialogue)
    }
}
