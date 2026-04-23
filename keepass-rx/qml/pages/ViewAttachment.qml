import QtQuick 2.12
import Lomiri.Components 1.3
import Qt.labs.settings 1.0
import keepassrx 1.0

Page {
    id: viewAttachmentPage
    property string attachmentName
    property string displayName
    property string mimeType
    property string viewType
    property string text
    property string dataUrl
    property var sourcePage

    header: PageHeader {
        id: header
        title: displayName || attachmentName || i18n.tr("Attachment")

        leadingActionBar.actions: [
            Action {
                name: "Close"
                text: i18n.tr("Close")
                iconName: "close"
                onTriggered: {
                    pageStack.removePages(viewAttachmentPage);
                }
            }
        ]

        trailingActionBar.actions: [
            Action {
                name: "Export"
                text: i18n.tr("Export")
                iconName: "document-save-as"
                onTriggered: {
                    if (sourcePage) {
                        sourcePage.beginAttachmentExport(attachmentName);
                    }
                    pageStack.removePages(viewAttachmentPage);
                }
            }
        ]
    }

    Loader {
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sourceComponent: viewType === "image" ? imageViewer : textViewer
    }

    Component {
        id: textViewer

        TextArea {
            anchors.fill: parent
            anchors.leftMargin: units.gu(1.0)
            anchors.rightMargin: units.gu(1.0)
            text: viewAttachmentPage.text
            readOnly: true
            cursorVisible: selectedText !== ""
            selectByMouse: true
            persistentSelection: true
            wrapMode: Text.Wrap
        }
    }

    Component {
        id: imageViewer

        Flickable {
            id: imageFlickable
            anchors.fill: parent
            clip: true
            contentWidth: imageContainer.width
            contentHeight: imageContainer.height
            boundsBehavior: Flickable.DragAndOvershootBounds

            property real minimumScale: 1
            property real maximumScale: 6
            property real imageScale: 1

            function fitImage() {
                if (attachmentImage.status !== Image.Ready || attachmentImage.implicitWidth === 0 || attachmentImage.implicitHeight === 0) {
                    return;
                }

                const widthScale = imageFlickable.width / attachmentImage.implicitWidth;
                const heightScale = imageFlickable.height / attachmentImage.implicitHeight;
                minimumScale = Math.min(1, widthScale, heightScale);
                imageScale = minimumScale;
                returnToBounds();
            }

            PinchArea {
                anchors.fill: parent
                pinch.minimumScale: imageFlickable.minimumScale
                pinch.maximumScale: imageFlickable.maximumScale

                onPinchUpdated: {
                    const nextScale = Math.max(
                        imageFlickable.minimumScale,
                        Math.min(imageFlickable.maximumScale, imageFlickable.imageScale * pinch.scale)
                    );
                    imageFlickable.imageScale = nextScale;
                }

                onPinchFinished: {
                    imageFlickable.returnToBounds();
                }

                Item {
                    id: imageContainer
                    width: Math.max(imageFlickable.width, attachmentImage.implicitWidth * imageFlickable.imageScale)
                    height: Math.max(imageFlickable.height, attachmentImage.implicitHeight * imageFlickable.imageScale)

                    Image {
                        id: attachmentImage
                        anchors.centerIn: parent
                        source: viewAttachmentPage.dataUrl
                        fillMode: Image.PreserveAspectFit
                        width: implicitWidth * imageFlickable.imageScale
                        height: implicitHeight * imageFlickable.imageScale
                        asynchronous: true
                        smooth: true

                        onStatusChanged: {
                            if (status === Image.Ready) {
                                imageFlickable.fitImage();
                            }
                        }
                    }
                }
            }

            onWidthChanged: fitImage()
            onHeightChanged: fitImage()
        }
    }
}
