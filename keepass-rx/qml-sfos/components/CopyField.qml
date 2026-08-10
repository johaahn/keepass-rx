/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation; version 3.
 */
import QtQuick 2.6
import Sailfish.Silica 1.0

// A clickable label/value row. Tapping it (or the copy icon) invokes the
// `clicked` handler, which by convention copies the field to the clipboard.
ListItem {
    id: field

    property string label
    property string value

    contentHeight: Math.max(Theme.itemSizeSmall, col.height + 2 * Theme.paddingSmall)

    Column {
        id: col
        anchors {
            left: parent.left
            leftMargin: Theme.horizontalPageMargin
            right: copyIcon.left
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
            text: field.value
            truncationMode: TruncationMode.Fade
            color: field.highlighted ? Theme.highlightColor : Theme.primaryColor
        }
    }

    Image {
        id: copyIcon
        anchors {
            right: parent.right
            rightMargin: Theme.horizontalPageMargin
            verticalCenter: parent.verticalCenter
        }
        source: "image://theme/icon-m-clipboard"
    }
}
