import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3
import QtGraphicalEffects 1.0

ListItem {
    property bool passwordVisible: false
    height: units.gu(10)
    id: entireItem

    function handleEntryClick() {
        if (itemType == 'Entry') {
            keepassrx.getSingleEntry(uuid);
        } else if (itemType == 'GoBack') {
            keepassrx.popContainer();
        } else {
          // We assume anything else is a grouping.
          keepassrx.pushContainer(uuid);
        }
    }

    function resolveFolderIconName(itemType) {
        if (itemType == 'Group' || itemType == 'Template') {
            return 'folder';
        } else if (itemType == 'Tag') {
            return 'tag';
        } else {
            return 'up';
        }
    }

    function resolveImagePath() {
        if (iconPath) {
            if (iconBuiltin) {
                return `../../assets/icons/${iconPath}`;
            } else {
                return iconPath;
            }
        } else {
            return '../../assets/placeholder.png';
        }
    }

    Connections {
        target: keepassrx

        // When the UI requests getting a single value from one of the
        // button presses.
        function onFieldValueReceived(entryUuid, fieldName, fieldValue, fieldExtra) {
            if (fieldValue) {
                // 2fa stuff handled by other signal.
                if (hasFeature('DisplayTwoFactorAuth')) {
                    return;
                }

                // TODO Add some better URL handling, for fields that
                // are not marked specifically with title "URL".
                if (fieldName.toLowerCase() == "url") {
                    if (entry.url.indexOf('//') === -1) {
                        Qt.openUrlExternally('http://' + url);
                        return;
                    }

                    Qt.openUrlExternally(url);
                } else {
                    Clipboard.push(fieldValue);
                    toast.show(`${fieldName} copied to clipboard (30 secs)`);
                    clearClipboardTimer.start();
                }
            }
        }

        function onSingleEntryReceived(entry) {
            if (entry) {
                pageStack.addPageToNextColumn(
                    entriesPage,
                    Qt.resolvedUrl("../pages/SingleEntry.qml"),
                    {
                        entryTitle: entry.title ? entry.title : null,
                        entryUsername: entry.username ? entry.username : null,
                        entryPassword: entry.password ? entry.password : null,
                        entryUrl: entry.url ? entry.url : null,
                        entryNotes: entry.notes ? entry.notes : null,
                        entryCustomFields: entry.customFields ? entry.customFields : null
                    }
                )
            }
        }
    }

    StyleHints {
        trailingPanelColor: theme.palette.normal.foreground
        trailingForegroundColor: theme.palette.normal.foregroundText
    }

    trailingActions: ListItemActions {
        actions: [
            Action {
                visible: hasUsername
                name: i18n.tr('Copy Username')
                iconName: "account"
                onTriggered: {
                    keepassrx.getFieldValue(uuid, "Username");
                }
            },
            Action {
                visible: hasPassword
                name: i18n.tr('Copy Password')
                iconName: "stock_key"
                onTriggered: {
                    keepassrx.getFieldValue(uuid, "Password");
                }
            },
            Action {
                visible: hasURL
                iconName: "external-link"
                name: i18n.tr('Open URL')
                onTriggered: {
                    keepassrx.getFieldValue(uuid, "URL");
                }
            },
            Action {
                visible: hasTOTP && !hasFeature('DisplayTwoFactorAuth')
                name: i18n.tr('Copy 2FA Code')
                iconSource: "../../assets/2fa.svg"
                iconName: "totp-code"
                onTriggered: {
                    // Need to fetch current TOTP because it shifts
                    // with time. Response is handled by the
                    // onTotpReceived event.
                    keepassrx.getTotp(uuid);
                }
            },
            Action {
                visible: hasFeature('DisplayTwoFactorAuth')
                name: i18n.tr('View Entry')
                iconName: "next"
                onTriggered: handleEntryClick()
            }
        ]
    }

    Rectangle {
        anchors.fill: parent
        color: theme.palette.normal.background
    }

    Row {
        anchors.leftMargin: units.gu(2)
        anchors.rightMargin: units.gu(2)
        anchors.topMargin: units.gu(1)
        anchors.bottomMargin: units.gu(1)
        anchors.fill: parent

        spacing: units.gu(1)

        // The Loader will return either a folder + custom image, or
        // just a custom image, depending on if we are a tag/group or
        // an entry.
        Loader {
            id: imgLoader
            width: units.gu(5)
            height: parent.height
            sourceComponent: itemType == 'Group' || itemType == 'Tag' ? folderImg : entryImg

            Component {
                id: folderImg

                Item {
                    width: units.gu(5)
                    height: parent.height

                    // The folder icon itself (groups + tags only, not templates)
                    Icon {
                        width: units.gu(5)
                        height: parent.height
                        y: parent.height / 2 - height / 2
                        name: resolveFolderIconName(itemType)
                    }

                    // Icon of the group/folder, if it has one.
                    Image {
                        id: groupEntryImg
                        visible: itemType !== 'Tag' // no tiny images for tags.
                        fillMode: Image.PreserveAspectFit
                        source: resolveImagePath()
                        width: units.gu(2.75)
                        height: units.gu(2.75)
                        y: parent.height - height * 1.5
                        x: parent.width - width / 1.25
                    }
                }
            }

            Component {
                id: entryImg

                Image {
                    fillMode: Image.PreserveAspectFit
                    source: resolveImagePath()
                    width: units.gu(5)
                    height: parent.height
                    y: parent.height / 2 - height / 2
                }
            }
        }

        Column {
            id: detailsColumn
            width: parent.width - parent.spacing - units.gu(12)

            Text {
                id: titleText
                width: parent.width
                elide: Text.ElideRight
                font.pointSize: units.gu(1.5)
                color: theme.palette.normal.foregroundText
                text: title
            }

            Text {
                width: parent.width
                elide: Text.ElideRight
                color: theme.palette.normal.backgroundTertiaryText
                text: subtitle
            }

            Text {
                elide: Text.ElideRight
                color: theme.palette.normal.activity
                text: hasFeature('DisplayTwoFactorAuth')
                    ? i18n.tr("Tap to copy 2FA code")
                    : (description)
            }
        }

        Loader {
            id: featureLoader
            width: parent.width - imgLoader.width - detailsColumn.width
            height: parent.height
            sourceComponent: hasFeature('DisplayTwoFactorAuth') ? totpFeature : noFeatures

            Component {
                id: totpFeature
                TotpFeature {
                    entryId: uuid
                }
            }

            Component {
                id: noFeatures
                NoFeatures {}
            }
        }
    } // end Features Loader


    // Main handler for "Doing The Thing" when tapping an entry.
    MouseArea {
        id: mainAction
        x: parent.x
        width: imgLoader.width + detailsColumn.width
        height: parent.height
        onClicked: {
            if (hasFeature('DisplayTwoFactorAuth')) {
                keepassrx.getTotp(uuid);
            } else {
                handleEntryClick();
            }
        }
    }

    Timer {
        id: timer
        repeat: false
        interval: 1500
        onTriggered: passwordVisible = false
    }

    function hasFeature(featureName) {
        return feature !== undefined && feature == featureName
    }
}
