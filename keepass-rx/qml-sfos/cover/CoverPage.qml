/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation; version 3.
 */
import QtQuick 2.6
import Sailfish.Silica 1.0

CoverBackground {
    Image {
        anchors.centerIn: parent
        width: parent.width * 0.6
        height: width
        fillMode: Image.PreserveAspectFit
        source: "../../assets/keepass-rx.svg"
    }

    Label {
        anchors {
            bottom: parent.bottom
            bottomMargin: Theme.paddingLarge
            horizontalCenter: parent.horizontalCenter
        }
        text: "KeePassRX"
        color: Theme.primaryColor
    }
}
