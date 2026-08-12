/*
 * Copyright (C) 2025 projectmoon
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation; version 3.
 */
import QtQuick 2.6
import Sailfish.Silica 1.0
import Sailfish.Pickers 1.0
import keepassrx 1.0

Page {
    id: viewPage
    allowedOrientations: defaultAllowedOrientations

    property string entryUuid
    property string attachmentName
    property string displayName
    property string mimeType
    property string viewType
    property string text: ""
    property string highlightedText: ""
    property string dataUrl: ""
    property string backgroundColor: ""
    property string foregroundColor: ""

    readonly property color textBackground: backgroundColor.length > 0
        ? backgroundColor
        : (Theme.colorScheme === Theme.LightOnDark ? "#1c1c1c" : "#ffffff")
    readonly property color textForeground: foregroundColor.length > 0
        ? foregroundColor
        : (Theme.colorScheme === Theme.LightOnDark ? "#ffffff" : "#000000")

    function exportTo(path) {
        var result = entryModel.exportAttachmentTo(viewPage.attachmentName, path);
        if (result.ok) {
            applicationWindow.notify(Tr.tr("Saved %1").arg(result.fileName));
        } else {
            applicationWindow.notify(result.error
                ? result.error : Tr.tr("Unable to export attachment."));
        }
    }

    RxUiEntry {
        id: entryModel
        entryUuid: viewPage.entryUuid
        app: AppState
    }

    Component {
        id: folderPicker
        FolderPickerPage {
            title: Tr.tr("Export to")
            onSelectedPathChanged: viewPage.exportTo(selectedPath)
        }
    }

    SilicaFlickable {
        id: flickable
        anchors.fill: parent
        contentHeight: viewType === "text" ? textColumn.height : height

        PullDownMenu {
            MenuItem {
                text: Tr.tr("Export")
                onClicked: pageStack.push(folderPicker)
            }
        }

        VerticalScrollDecorator {}

        Column {
            id: textColumn
            width: parent.width
            visible: viewPage.viewType === "text"

            PageHeader {
                title: viewPage.displayName
            }

            Rectangle {
                width: parent.width
                height: previewLabel.height + 2 * Theme.paddingMedium
                color: viewPage.textBackground

                Label {
                    id: previewLabel
                    x: Theme.horizontalPageMargin
                    y: Theme.paddingMedium
                    width: parent.width - 2 * Theme.horizontalPageMargin
                    wrapMode: Text.Wrap
                    textFormat: viewPage.highlightedText.length > 0
                        ? Text.RichText : Text.PlainText
                    font.family: "monospace"
                    font.pixelSize: Theme.fontSizeExtraSmall
                    color: viewPage.textForeground
                    text: viewPage.highlightedText.length > 0
                        ? viewPage.highlightedText : viewPage.text
                }
            }
        }

        Image {
            id: imageView
            visible: viewPage.viewType === "image"
            anchors.fill: parent
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            smooth: true
            source: viewPage.viewType === "image" ? viewPage.dataUrl : ""
        }

        ViewPlaceholder {
            enabled: viewPage.viewType !== "text" && viewPage.viewType !== "image"
            text: Tr.tr("Cannot preview this file type")
            hintText: Tr.tr("Pull down to export the attachment.")
        }
    }
}
