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
    id: item

    property string attachmentName
    property int attachmentSize
    property string attachmentMimeType

    signal activated(string name)

    contentHeight: Theme.itemSizeMedium

    onClicked: item.activated(attachmentName)

    Image {
        id: icon
        anchors.verticalCenter: parent.verticalCenter
        x: Theme.horizontalPageMargin
        width: Theme.iconSizeMedium
        height: Theme.iconSizeMedium
        fillMode: Image.PreserveAspectFit
        source: "image://theme/icon-m-attach"
    }

    Column {
        anchors {
            left: icon.right
            leftMargin: Theme.paddingMedium
            right: parent.right
            rightMargin: Theme.horizontalPageMargin
            verticalCenter: parent.verticalCenter
        }

        Label {
            width: parent.width
            truncationMode: TruncationMode.Fade
            text: item.attachmentName
            color: item.highlighted ? Theme.highlightColor : Theme.primaryColor
        }

        Label {
            width: parent.width
            truncationMode: TruncationMode.Fade
            visible: text.length > 0
            text: {
                var size = Tr.tr("%1 bytes").arg(item.attachmentSize);
                return item.attachmentMimeType.length > 0
                    ? size + " · " + item.attachmentMimeType
                    : size;
            }
            font.pixelSize: Theme.fontSizeExtraSmall
            color: item.highlighted ? Theme.secondaryHighlightColor : Theme.secondaryColor
        }
    }
}
