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
    property string entryUsername: ""
    property string entryPassword: ""
    property string entryUrl: ""
    property string entryNotes: ""
    property bool entryHasUsername: false
    property bool entryHasPassword: false
    property bool entryHasUrl: false
    property bool entryHasNotes: false
    property var entryCustomFields
    property bool entryHasTotp: false
    property var entryEntropy: null
    property string entryEntropyQuality
    property var colorWashout

    property string entryUsernameValue: ""
    property string entryPasswordValue: ""
    property string entryUrlValue: ""
    property string entryNotesValue: ""
    property bool notesShown: false

    function valueIsDefined(value) {
        return value !== undefined && value !== null && value !== ''
    }

    function hasTotpSection() {
        return entryHasTotp && valueIsDefined(entryUuid)
    }

    function hasEntropySection() {
        return (entryHasPassword || valueIsDefined(entryPassword))
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
        toast.show(i18n.tr('%1 copied to clipboard (30 secs)').arg(fieldName));
        clearClipboardTimer.start();
    }

    function revealedValue(fieldName) {
        switch (fieldName) {
        case "Username":
            return valueIsDefined(entryUsernameValue) ? entryUsernameValue : entryUsername;
        case "Password":
            return valueIsDefined(entryPasswordValue) ? entryPasswordValue : entryPassword;
        case "URL":
            return valueIsDefined(entryUrlValue) ? entryUrlValue : entryUrl;
        case "Notes":
            return valueIsDefined(entryNotesValue) ? entryNotesValue : entryNotes;
        default:
            return "";
        }
    }

    function clearFieldValue(fieldName) {
        switch (fieldName) {
        case "Username":
            entryUsernameValue = "";
            break;
        case "Password":
            entryPasswordValue = "";
            break;
        case "URL":
            entryUrlValue = "";
            break;
        case "Notes":
            entryNotesValue = "";
            notesShown = false;
            break;
        default:
            clearCustomFieldValue(fieldName);
            break;
        }
    }

    function revealFieldValue(fieldName) {
        if (valueIsDefined(entryUuid)) {
            keepassrx.revealFieldValue(entryUuid, fieldName);
        }
    }

    function copyFieldValue(fieldName) {
        const fieldValue = revealedValue(fieldName);
        if (valueIsDefined(fieldValue)) {
            copyToClipboard(fieldName, fieldValue);
        } else if (valueIsDefined(entryUuid)) {
            keepassrx.getFieldValue(entryUuid, fieldName);
        }
    }

    function findCustomFieldIndex(fieldName) {
        for (let i = 0; i < otherFieldsModel.count; i++) {
            if (otherFieldsModel.get(i).fieldName === fieldName) {
                return i;
            }
        }

        return -1;
    }

    function setCustomFieldValue(fieldName, fieldValue) {
        const index = findCustomFieldIndex(fieldName);
        if (index !== -1) {
            otherFieldsModel.setProperty(index, "fieldValue", fieldValue);
        }
    }

    function setCustomFieldShown(fieldName, isShown) {
        const index = findCustomFieldIndex(fieldName);
        if (index !== -1) {
            otherFieldsModel.setProperty(index, "fieldShown", isShown);
        }
    }

    function clearCustomFieldValue(fieldName) {
        const index = findCustomFieldIndex(fieldName);
        if (index !== -1) {
            otherFieldsModel.setProperty(index, "fieldValue", "");
            otherFieldsModel.setProperty(index, "fieldShown", false);
        }
    }

    function clearTransientValues() {
        entryUsernameValue = "";
        entryPasswordValue = "";
        entryUrlValue = "";
        entryNotesValue = "";
        notesShown = false;

        for (let i = 0; i < otherFieldsModel.count; i++) {
            otherFieldsModel.setProperty(i, "fieldValue", "");
            otherFieldsModel.setProperty(i, "fieldShown", false);
        }
    }

    Component.onCompleted: {
        const metadata = keepassrx.metadata;

        if (metadata.publicColor) {
            colorWashout = keepassrx.washOutColor(metadata.publicColor);
        }

        if (entryCustomFields) {
            for (const [key, field] of Object.entries(entryCustomFields)) {
                otherFieldsModel.append({
                    fieldName: key,
                    fieldValue: field.value ? field.value : "",
                    fieldShown: field.isHiddenByDefault !== true,
                    fieldHiddenByDefault: field.isHiddenByDefault === true
                });
            }
        }
    }

    Component.onDestruction: clearTransientValues()
    onVisibleChanged: {
        if (!visible) {
            clearTransientValues();
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

    Connections {
        target: keepassrx

        function onFieldValueReceived(entryUuid, fieldName, fieldValue, fieldExtra) {
            if (fieldExtra !== "reveal" || entryUuid !== singleEntryPage.entryUuid) {
                return;
            }

            switch (fieldName) {
            case "Username":
                if (usernameField.isContentVisible) {
                    entryUsernameValue = fieldValue;
                }
                break;
            case "Password":
                if (passwordField.isContentVisible) {
                    entryPasswordValue = fieldValue;
                }
                break;
            case "URL":
                if (urlField.isContentVisible) {
                    entryUrlValue = fieldValue;
                }
                break;
            case "Notes":
                if (notesShown) {
                    entryNotesValue = fieldValue;
                }
                break;
            default: {
                const index = findCustomFieldIndex(fieldName);
                if (index !== -1 && otherFieldsModel.get(index).fieldShown) {
                    setCustomFieldValue(fieldName, fieldValue);
                }
                break;
            }
            }
        }

        function onDatabaseClosed() {
            clearTransientValues();
        }

        function onMasterPasswordInvalidated() {
            clearTransientValues();
        }

        function onMasterPasswordStateChanged(encrypted) {
            if (encrypted) {
                clearTransientValues();
            }
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
                    clearTransientValues();
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
        visible: entryHasNotes
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
                visible: entryHasNotes && !valueIsDefined(entryNotes)
                text: valueIsDefined(entryNotesValue) ? i18n.tr("Tap to hide") : i18n.tr("Tap to reveal")
                color: LomiriColors.slate
                textSize: Label.Small
                Layout.alignment: Qt.AlignRight

                MouseArea {
                    z: 10
                    anchors.fill: parent
                    onClicked: {
                        if (valueIsDefined(entryNotesValue)) {
                            clearFieldValue("Notes");
                        } else {
                            notesShown = true;
                            revealFieldValue("Notes");
                        }
                    }
                }
            }

            Label {
                Layout.preferredHeight: notesLabel.height
                text: i18n.tr("Tap to copy")
                color: LomiriColors.slate
                textSize: Label.Small
                Layout.alignment: Qt.AlignRight

                MouseArea {
                    z: 10
                    anchors.fill: parent
                    onClicked: copyFieldValue("Notes")
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
                text: valueIsDefined(entryNotes) ? entryNotes
                    : (valueIsDefined(entryNotesValue) ? entryNotesValue : i18n.tr("Tap reveal to view notes"))
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
            visible: entryHasUsername
                || entryHasPassword
                || entryHasUrl
                || valueIsDefined(entryUsername)
                || valueIsDefined(entryPassword)
                || valueIsDefined(entryUrl)

            DetailField {
                id: usernameField
                title: i18n.tr("Username")
                visible: entryHasUsername
                subtitle: valueIsDefined(entryUsername) ? entryUsername : entryUsernameValue
                visibleContent: valueIsDefined(entryUsername) ? entryUsername
                    : (valueIsDefined(entryUsernameValue) ? entryUsernameValue : i18n.tr("Loading…"))
                hiddenContent: i18n.tr("Tap to reveal")
                showVisibilityToggle: !valueIsDefined(entryUsername)
                isContentVisible: valueIsDefined(entryUsername)
                onVisibilityToggled: {
                    if (isContentVisible) {
                        revealFieldValue("Username")
                    } else {
                        clearFieldValue("Username")
                    }
                }
                onCopyClicked: copyFieldValue("Username")
                showDivider: entryHasPassword
                    || entryHasUrl
                    || hasTotpSection()
            }

            DetailField {
                id: passwordField
                title: passwordFieldTitle()
                visible: entryHasPassword
                subtitle: valueIsDefined(entryPassword) ? entryPassword : entryPasswordValue
                visibleContent: valueIsDefined(entryPassword) ? entryPassword
                    : (valueIsDefined(entryPasswordValue) ? entryPasswordValue : i18n.tr("Loading…"))
                hiddenContent: "••••••••••••"
                showVisibilityToggle: !valueIsDefined(entryPassword)
                isContentVisible: valueIsDefined(entryPassword)
                onVisibilityToggled: {
                    if (isContentVisible) {
                        revealFieldValue("Password")
                    } else {
                        clearFieldValue("Password")
                    }
                }
                onCopyClicked: copyFieldValue("Password")
                showDivider: entryHasUrl || hasTotpSection()
            }

            DetailField {
                id: urlField
                visible: entryHasUrl
                title: i18n.tr("URL")
                subtitle: valueIsDefined(entryUrl) ? entryUrl : entryUrlValue
                visibleContent: valueIsDefined(entryUrl) ? entryUrl
                    : (valueIsDefined(entryUrlValue) ? entryUrlValue : i18n.tr("Loading…"))
                hiddenContent: i18n.tr("Tap to reveal")
                showVisibilityToggle: !valueIsDefined(entryUrl)
                isContentVisible: valueIsDefined(entryUrl)
                onVisibilityToggled: {
                    if (isContentVisible) {
                        revealFieldValue("URL")
                    } else {
                        clearFieldValue("URL")
                    }
                }
                onCopyClicked: copyFieldValue("URL")
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
                    visibleContent: valueIsDefined(fieldValue) ? fieldValue : i18n.tr("Loading…")
                    hiddenContent: fieldHiddenByDefault ? "••••••••••••" : i18n.tr("Tap to reveal")
                    showVisibilityToggle: fieldHiddenByDefault
                    isContentVisible: fieldShown
                    showDivider: index < otherFieldsRepeater.count - 1
                    onVisibilityToggled: {
                        if (isContentVisible) {
                            setCustomFieldShown(fieldName, true)
                            revealFieldValue(fieldName)
                        } else {
                            clearCustomFieldValue(fieldName)
                        }
                    }
                    onCopyClicked: {
                        if (valueIsDefined(fieldValue)) {
                            copyToClipboard(fieldName, fieldValue)
                        } else if (valueIsDefined(entryUuid)) {
                            keepassrx.getFieldValue(entryUuid, fieldName)
                        }
                    }
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
