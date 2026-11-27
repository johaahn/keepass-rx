import QtQuick 2.12
import Lomiri.Components 1.3

ListItem {
    id: notesItem
    property string notesText
    property bool modified

    height: notesLayout.height

    ListItemLayout {
        id: notesLayout
        title.text: notesItem.modified ? i18n.tr("Notes (modified)") : i18n.tr("Notes")
        title.color: notesItem.modified ? theme.palette.normal.activity : theme.palette.normal.backgroundText
        subtitle.text: notesItem.notesText.length > 0 ? notesItem.notesText : i18n.tr("No notes")
        subtitle.maximumLineCount: 3
        subtitle.wrapMode: Text.Wrap

        Icon {
            name: "next"
            SlotsLayout.overrideVerticalPositioning: true
            SlotsLayout.position: SlotsLayout.Trailing
            width: units.gu(3)
            height: units.gu(3)
            anchors.verticalCenter: parent.verticalCenter
        }
    }
}
