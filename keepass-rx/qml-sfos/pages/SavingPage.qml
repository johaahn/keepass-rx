import QtQuick 2.6
import Sailfish.Silica 1.0
import keepassrx 1.0

Page {
    id: savingPage
    allowedOrientations: defaultAllowedOrientations
    backNavigation: !keepassrx.saving

    property var editor
    property bool started: false
    property string outcome: ""

    function finish(result) {
        outcome = result;
        settle();
    }

    function settle() {
        if (status !== PageStatus.Active || pageStack.busy) {
            return;
        }
        if (outcome === "saved") {
            pageStack.pop(pageStack.previousPage(editor));
        } else if (outcome === "back") {
            pageStack.pop(editor);
        }
    }

    onStatusChanged: {
        if (status !== PageStatus.Active) {
            return;
        }
        if (!started) {
            started = true;
            editor.beginSave(savingPage);
        }
        settle();
    }

    Component.onDestruction: {
        if (editor && editor.savingPage === savingPage) {
            editor.savingPage = null;
        }
    }

    Connections {
        target: pageStack
        onBusyChanged: savingPage.settle()
    }

    BusyLabel {
        text: Tr.tr("Saving... Do not close the app.")
        running: true
    }
}
