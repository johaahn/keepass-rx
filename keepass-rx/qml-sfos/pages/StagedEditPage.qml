import QtQuick 2.6
import Sailfish.Silica 1.0

Page {
    id: page
    property string entryTitle
    property bool entryChanged: false
    property string error
    property bool busy: false

    backNavigation: false

    function fieldLabel(name, custom) {
        if (custom) return name;
        if (name === "title") return Tr.tr("Title");
        if (name === "username") return Tr.tr("Username");
        if (name === "password") return Tr.tr("Password");
        if (name === "url") return Tr.tr("URL");
        return Tr.tr("Notes");
    }

    function kindLabel(kind) {
        if (kind === "added") return Tr.tr("Added");
        if (kind === "deleted") return Tr.tr("Deleted");
        return Tr.tr("Modified");
    }

    function resolve(choice) {
        busy = true;
        keepassrx.resolveStagedEdit(choice);
    }

    Connections {
        target: keepassrx
        onStagedEditFailed: page.busy = false
    }

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: column.height

        Column {
            id: column
            width: parent.width
            spacing: Theme.paddingLarge

            PageHeader {
                title: page.error ? Tr.tr("Keep unsaved changes?") : Tr.tr("Apply unsaved changes?")
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                wrapMode: Text.Wrap
                color: Theme.highlightColor
                text: page.error
                    ? Tr.tr("Your unsaved changes could not be read: %1. Do you want to keep them and try again later, or discard them?").arg(page.error)
                    : Tr.tr("KeePassRX closed before your changes to %1 were saved. Do you want to apply them now?").arg(page.entryTitle)
            }

            Column {
                width: parent.width
                visible: !page.error

                Repeater {
                    model: keepassrx.stagedChanges

                    DetailItem {
                        label: page.fieldLabel(model.name, model.custom)
                        value: page.kindLabel(model.kind)
                    }
                }
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                visible: page.entryChanged
                wrapMode: Text.Wrap
                color: Theme.errorColor
                text: Tr.tr("This entry was also changed somewhere else after your edit. Applying replaces those changes, and the replaced values stay in the entry history.")
            }

            Button {
                anchors.horizontalCenter: parent.horizontalCenter
                enabled: !page.busy
                text: page.error ? Tr.tr("Keep") : Tr.tr("Apply")
                onClicked: page.resolve(page.error ? "keep" : "apply")
            }

            Button {
                anchors.horizontalCenter: parent.horizontalCenter
                enabled: !page.busy
                text: Tr.tr("Discard")
                onClicked: page.resolve("discard")
            }

            BusyIndicator {
                anchors.horizontalCenter: parent.horizontalCenter
                size: BusyIndicatorSize.Medium
                running: page.busy
            }

            Label {
                x: Theme.horizontalPageMargin
                width: parent.width - 2 * Theme.horizontalPageMargin
                horizontalAlignment: Text.AlignHCenter
                visible: keepassrx.saving
                color: Theme.secondaryHighlightColor
                text: Tr.tr("Saving... Do not close the app.")
            }
        }
    }
}
