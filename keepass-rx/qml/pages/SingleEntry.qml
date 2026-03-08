import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Layouts 1.3
import Lomiri.Components 1.3
import keepassrx 1.0

import "../components"

Page {
    id: singleEntryPage
    property string entryUuid
    property string entryTitle
    property string entryUsername
    property string entryPassword
    property string entryUrl
    property string entryNotes
    property var entryCustomFields
    property bool entryHasTotp: false
    property var entryEntropy: null
    property string entryEntropyQuality
    property var colorWashout

    function valueIsDefined(value) {
        return value !== undefined && value !== null && value !== ''
    }

    function hasTotpSection() {
        return entryHasTotp && valueIsDefined(entryUuid)
    }

    function hasEntropySection() {
        return valueIsDefined(entryPassword)
            && entryEntropy !== undefined
            && entryEntropy !== null
            && !Number.isNaN(Number(entryEntropy))
    }

    function entropyBitsColor(bits) {
        if (bits < 40) {
            return LomiriColors.red
        } else if (bits < 75) {
            return LomiriColors.orange
        } else if (bits < 100) {
            return Theme.name == "Lomiri.Components.Themes.Ambiance"
                ? LomiriColors.green
                : LomiriColors.lightGreen
        }
        return LomiriColors.green
    }

    function passwordFieldTitle() {
        if (!hasEntropySection()) {
            return i18n.tr("Password")
        }

        const bits = Number(entryEntropy).toFixed(2);
        const coloredBits = "<font color=\"" + entropyBitsColor(Number(entryEntropy)) + "\">"
              + i18n.tr('%1 bits').arg(bits)
              + "</font>";
        return i18n.tr("Password (Entropy: %1)").arg(coloredBits);
    }

    function copyToClipboard(fieldName, fieldValue) {
        Clipboard.push(fieldValue);
        toast.show(i18n.tr(`%1 copied to clipboard (30 secs)`).arg(fieldName));
        clearClipboardTimer.start();
    }

    Component.onCompleted: {
        const metadata = keepassrx.metadata;

        if (metadata.publicColor) {
            colorWashout = keepassrx.washOutColor(metadata.publicColor);
        }

        if (entryCustomFields) {
            // value is { value: string, isHiddenByDefault: bool }
            for (const [key, field] of Object.entries(entryCustomFields)) {
                otherFieldsModel.append({
                    fieldName: key,
                    fieldValue: field.value,
                    fieldShown: !field.isHiddenByDefault
                });
            }
        }
    }

    function headerBackgroundColor() {
        if (SettingsBridge.showAccents && colorWashout) {
            return colorWashout.backgroundColor;
        } else {
            return "transparent";
        }
    }

    function headerTextColor() {
        if (SettingsBridge.showAccents && colorWashout) {
            // textColorType is the color type for the header text itself.
            return colorWashout.textColorType === 'Light'
                ? LomiriColors.white
                : LomiriColors.jet;
        } else {
            return theme.palette.normal.foregroundText;
        }
    }

    header: PageHeader {
        title: entryTitle || i18n.ctr('Page header for single entry', 'Untitled Entry')

        StyleHints {
            backgroundColor: headerBackgroundColor()
            foregroundColor: headerTextColor()
        }

        // For some reason the auto-managed back button isn't showing
        // up, so we make our own.
        leadingActionBar.actions: [
            Action {
                name: "Back"
                text: i18n.tr("Back")
                iconName: "previous"
                onTriggered: {
                    // If we remove primary page, only child pages
                    // (i.e. THIS page) are removed. So, this sends us
                    // back to entries list.
                    pageStack.removePages(pageStack.primaryPage);
                }
            }
        ]
    }

    ListModel {
        id: otherFieldsModel
    }

    RxUiEntry {
        id: totpEntry
        entryUuid: singleEntryPage.entryUuid
        app: AppState
    }

    Timer {
        id: currentTotpTimer
        repeat: true
        interval: 1000
        running: hasTotpSection()
        triggeredOnStart: true
        onTriggered: {
            if (hasTotpSection()) {
                totpEntry.updateTotp();
            }
        }
    }

    Rectangle {
        color: Theme.name == "Lomiri.Components.Themes.Ambiance" ? LomiriColors.porcelain : LomiriColors.inkstone
        visible: entryNotes && entryNotes.length > 0
        id: notesComponent
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: units.gu(0.25)
        anchors.rightMargin: units.gu(0.25)
        height: units.gu(13)

        RowLayout {
            id: notesLabelRow

            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: units.gu(0.25)
            anchors.rightMargin: units.gu(0.25)

            Label {
                id: notesLabel
                Layout.fillWidth: true
                Layout.fillHeight: true
                // TRANSLATORS: This is a field from the KeePass database, which holds arbitrary text.
                text: i18n.tr("Notes")
                color: LomiriColors.orange
                textSize: Label.Large
            }

            Label {
                Layout.preferredHeight: notesLabel.height
                // TRANSLATORS: Pressing this will copy the Notes field of the entry.
                text: i18n.tr("Tap to copy")
                color: LomiriColors.slate
                textSize: Label.Small
                Layout.alignment: Qt.AlignRight

                MouseArea {
                    z: 10 // to make sure anywhere in the box is copyable
                    anchors.fill: parent
                    onClicked: {
                        Clipboard.push(entryNotes);
                        toast.show(i18n.tr('Notes copied to clipboard (30 secs)'));
                        clearClipboardTimer.start();
                    }
                }
            }
        }

        ScrollView {
            anchors.top: notesLabelRow.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: units.gu(0.25)
            anchors.rightMargin: units.gu(0.25)
            height: units.gu(8)
            width: parent.width
            id: notesContentRow
            clip: true

            Text {
                width: notesContentRow.width
                text: entryNotes ? entryNotes : ''
                wrapMode: Text.WordWrap
                color: LomiriColors.ash
                verticalAlignment: Text.AlignTop
            }
        }
    }

    Row {
        id: notesDivider
        visible: notesComponent.visible
        anchors.top: notesComponent.visible ? notesComponent.bottom : header.bottom
        anchors.left: parent.left
        anchors.right: parent.right

        Rectangle {
            width: parent.width
            height: 1
            color: LomiriColors.orange
        }
    }

    Row {
        id: standardFieldsRow
        anchors.top: notesComponent.visible ? notesDivider.bottom : header.bottom
        anchors.left: parent.left
        anchors.right: parent.right

        ConfigurationGroup {
            title: i18n.tr("Main")
            visible: valueIsDefined(entryUsername)
                || valueIsDefined(entryPassword)
                || valueIsDefined(entryUrl)

            DetailField {
                title: i18n.tr("Username")
                visible: valueIsDefined(entryUsername)
                subtitle: entryUsername
                onCopyClicked: copyToClipboard(i18n.tr("Username"), entryUsername)
                showDivider: valueIsDefined(entryPassword)
                    || valueIsDefined(entryUrl)
                    || hasTotpSection()
            }

            DetailField {
                title: passwordFieldTitle()
                visible: valueIsDefined(entryPassword)
                visibleContent: entryPassword
                showVisibilityToggle: true
                isContentVisible: false
                onCopyClicked: copyToClipboard(i18n.tr("Password"), entryPassword)
                showDivider: valueIsDefined(entryUrl) || hasTotpSection()
            }

            DetailField {
                visible: valueIsDefined(entryUrl)
                title: i18n.tr("URL")
                subtitle: entryUrl
                onCopyClicked: copyToClipboard(i18n.tr('URL'), entryUrl)
                showDivider: hasTotpSection()
            }

            DetailField {
                title: valueIsDefined(totpEntry.currentTotpValidFor)
                    ? i18n.tr("TOTP (Valid for %1)").arg(totpEntry.currentTotpValidFor)
                    : i18n.tr("TOTP")
                visible: hasTotpSection()
                subtitle: totpEntry.currentTotp
                showCopyButton: valueIsDefined(totpEntry.currentTotp)
                onCopyClicked: copyToClipboard(i18n.tr("TOTP"), totpEntry.currentTotp)
            }

        }
    }

    Flickable {
        id: customFieldsRow
        anchors.top: standardFieldsRow.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        width: parent.width
        Layout.fillWidth: true
        contentHeight: otherFields.height
        contentWidth: otherFields.width
        boundsMovement: Flickable.StopAtBounds
        clip: true

        ConfigurationGroup {
            id: otherFields
            visible: entryCustomFields && Object.entries(entryCustomFields).length > 0
            title: i18n.tr("Other Fields")

            Repeater {
                id: otherFieldsRepeater
                model: otherFieldsModel

                DetailField {
                    title: fieldName
                    subtitle: fieldValue
                    visibleContent: fieldValue
                    showVisibilityToggle: !fieldShown
                    isContentVisible: fieldShown
                    showDivider: index < otherFieldsRepeater.count - 1
                    onCopyClicked: copyToClipboard(fieldName, fieldValue)
                }
            }
        }
    }

    Timer {
        id: clearClipboardTimer
        repeat: false
        running: false
        interval: 30000
        onTriggered: {
            Clipboard.clear();
            toast.show(i18n.tr('KeePassRX: Clipboard cleared.'));
        }
    }

    Popup {
        id: toast
        padding: units.dp(12)

        x: parent.width / 2 - width / 2
        y: parent.height - height - units.dp(20)

        background: Rectangle {
            color: "#111111"
            opacity: 0.7
            radius: units.dp(10)
        }

        Text {
            id: popupLabel
            anchors.fill: parent
            horizontalAlignment: Text.AlignHCenter
            color: "#ffffff"
            font.pixelSize: units.dp(14)
        }

        Timer {
            id: popupTimer
            interval: 3000
            running: true
            onTriggered: {
                toast.close()
            }
        }

        function show(text) {
            popupLabel.text = text
            open()
            popupTimer.start()
        }
    }
}
