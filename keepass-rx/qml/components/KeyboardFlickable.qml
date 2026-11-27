import QtQuick 2.12
import QtQuick.Window 2.2
import Lomiri.Components 1.3

Flickable {
    id: flickable

    readonly property var hostWindow: Window.window

    function contains(item) {
        for (let node = item; node; node = node.parent) {
            if (node === flickable.contentItem) {
                return true;
            }
        }
        return false;
    }

    function revealFocus() {
        const item = hostWindow ? hostWindow.activeFocusItem : null;
        if (!item || !contains(item)) {
            return;
        }
        const margin = units.gu(2);
        const top = item.mapToItem(flickable.contentItem, 0, 0).y;
        const bottom = top + item.height;
        if (bottom + margin > flickable.contentY + flickable.height) {
            flickable.contentY = bottom + margin - flickable.height;
        } else if (top - margin < flickable.contentY) {
            flickable.contentY = Math.max(0, top - margin);
        }
    }

    onHeightChanged: revealTimer.restart()

    Connections {
        target: flickable.hostWindow

        function onActiveFocusItemChanged() {
            revealTimer.restart();
        }
    }

    Timer {
        id: revealTimer
        interval: 50
        onTriggered: flickable.revealFocus()
    }
}
