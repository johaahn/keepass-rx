/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation; version 3.
 */
import QtQuick 2.6
import Sailfish.Silica 1.0

ListItem {
    id: field

    property string label
    property string value
    property bool sensitive: false
    property bool revealed: false

    signal toggle()
    signal copy()

    contentHeight: Math.max(Theme.itemSizeSmall, col.height + 2 * Theme.paddingSmall)

    onClicked: if (field.sensitive) field.toggle()

    Column {
        id: col
        anchors {
            left: parent.left
            leftMargin: Theme.horizontalPageMargin
            right: revealIcon.visible ? revealIcon.left : parent.right
            rightMargin: Theme.paddingMedium
            verticalCenter: parent.verticalCenter
        }

        Label {
            width: parent.width
            text: field.label
            font.pixelSize: Theme.fontSizeExtraSmall
            color: field.highlighted ? Theme.secondaryHighlightColor : Theme.secondaryColor
        }

        Label {
            width: parent.width
            wrapMode: Text.WrapAnywhere
            text: (field.sensitive && !field.revealed) ? "••••••" : field.value
            color: field.highlighted ? Theme.highlightColor : Theme.primaryColor
        }
    }

    Image {
        id: revealIcon
        visible: field.sensitive
        anchors {
            right: parent.right
            rightMargin: Theme.horizontalPageMargin
            verticalCenter: parent.verticalCenter
        }
        source: field.revealed ? "image://theme/icon-m-hide" : "image://theme/icon-m-show"
    }

    menu: ContextMenu {
        MenuItem {
            text: Tr.tr("Copy to clipboard")
            onClicked: field.copy()
        }
        MenuItem {
            visible: field.sensitive
            text: field.revealed ? Tr.tr("Hide") : Tr.tr("Show")
            onClicked: field.toggle()
        }
    }
}
