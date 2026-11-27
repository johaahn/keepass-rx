import QtQuick 2.12
import QtQuick.Layouts 1.3
import Lomiri.Components 1.3
import Lomiri.Components.Popups 1.3
import Lomiri.Components.ListItems 1.3 as ListItems
import keepassrx 1.0

import "../components"

Page {
    id: editEntryPage
    property var entry
    property bool saveRequested: false
    property bool draftSaved: false
    property bool cleared: false
    property bool passwordShown: false
    property string notesText: entry.notes
    property int newFieldIndex: -1
    property var removedFields: []
    property var savingDialog: null

    function openSavingDialog() {
        if (!savingDialog) {
            savingDialog = PopupUtils.open(savingDialogComponent, editEntryPage);
        }
    }

    function closeSavingDialog() {
        if (savingDialog) {
            PopupUtils.close(savingDialog);
            savingDialog = null;
        }
    }

    function addField() {
        editableFieldsModel.append({
            originalName: "",
            fieldName: "",
            fieldValue: "",
            fieldProtected: false,
            valueShown: false,
            nameError: ""
        });
        newFieldIndex = editableFieldsModel.count - 1;
    }

    function showNewField() {
        const field = fieldRepeater.itemAt(newFieldIndex);
        if (!field) {
            return;
        }
        newFieldIndex = -1;
        const bottom = formFlickable.contentHeight - formFlickable.height;
        formFlickable.contentY = Math.max(0, Math.min(field.mapToItem(formColumn, 0, 0).y + formColumn.y, bottom));
        field.focusName();
    }

    function collectValues() {
        const customFields = [];
        for (let i = 0; i < editableFieldsModel.count; i++) {
            const field = editableFieldsModel.get(i);
            customFields.push({
                originalName: field.originalName,
                name: field.fieldName,
                value: field.fieldValue,
                protected: field.fieldProtected
            });
        }

        return {
            title: titleField.text,
            username: usernameField.text,
            password: passwordField.text,
            url: urlField.text,
            notes: notesText,
            customFields: customFields,
            removedFields: removedFields
        };
    }

    function clearValues() {
        cleared = true;
        titleField.text = "";
        usernameField.text = "";
        passwordField.text = "";
        urlField.text = "";
        notesText = "";
        editableFieldsModel.clear();
        readOnlyFieldsModel.clear();
        removedFields = [];
        entry = { uuid: entry.uuid };
    }

    function showProblems(problems) {
        for (let i = 0; i < editableFieldsModel.count; i++) {
            editableFieldsModel.setProperty(i, "nameError", "");
        }

        let first = -1;
        for (const problem of problems) {
            editableFieldsModel.setProperty(problem.index, "nameError", problem.message);
            if (first === -1 || problem.index < first) {
                first = problem.index;
            }
        }

        if (first !== -1) {
            scrollToRow(first);
        }
    }

    function scrollToRow(index) {
        const row = fieldRepeater.itemAt(index);
        if (!row) {
            return;
        }

        const top = row.mapToItem(formColumn, 0, 0).y + formColumn.y - units.gu(2);
        const maxY = Math.max(0, formFlickable.contentHeight - formFlickable.height);
        formFlickable.contentY = Math.max(0, Math.min(top, maxY));
    }

    function save() {
        const problems = keepassrx.validateEntryEdit(collectValues());
        if (problems.length > 0) {
            showProblems(problems);
            return;
        }

        if (keepassrx.saveAuthMethod === "shortCode") {
            askForCode(false, 0);
        } else {
            submit("");
        }
    }

    function askForCode(rejected, triesLeft) {
        PopupUtils.open(codeDialog, editEntryPage, {
            rejected: rejected,
            triesLeft: triesLeft
        });
    }

    function saveDraft() {
        if (cleared) {
            return;
        }
        draftSaved = true;
        keepassrx.saveDraft(entry.uuid, collectValues());
    }

    function submit(shortCode) {
        if (cleared) {
            return;
        }
        saveRequested = true;
        keepassrx.saveEntry(entry.uuid, collectValues(), shortCode);
    }

    function removeField(index) {
        const original = editableFieldsModel.get(index).originalName;
        if (original !== "") {
            removedFields = removedFields.concat([original]);
        }
        editableFieldsModel.remove(index);
    }

    Component.onCompleted: {
        for (const field of entry.customFields || []) {
            if (field.editable) {
                editableFieldsModel.append({
                    originalName: field.originalName ?? field.name,
                    fieldName: field.name,
                    fieldValue: field.value ?? "",
                    fieldProtected: field.protected,
                    valueShown: false,
                    nameError: ""
                });
            } else {
                readOnlyFieldsModel.append({
                    fieldName: field.name,
                    fieldValue: field.protected ? "••••••••••••" : field.value
                });
            }
        }
        removedFields = entry.removedFields || [];
        root.entryEditor = editEntryPage;
    }

    Component.onDestruction: {
        if (root.entryEditor === editEntryPage) {
            root.entryEditor = null;
        }
        if (!draftSaved && entry) {
            keepassrx.discardDraft(entry.uuid);
        }
    }

    header: PageHeader {
        title: i18n.tr("Edit Entry")

        leadingActionBar.actions: [
            Action {
                name: "Back"
                text: i18n.tr("Back")
                iconName: "previous"
                enabled: !keepassrx.saving
                onTriggered: pageStack.removePages(editEntryPage)
            }
        ]

        trailingActionBar.actions: [
            Action {
                name: "Save"
                text: i18n.tr("Save")
                iconName: "ok"
                enabled: !keepassrx.saving
                onTriggered: editEntryPage.save()
            },
            Action {
                name: "AddField"
                text: i18n.tr("Add field")
                iconName: "add"
                enabled: !keepassrx.saving
                onTriggered: editEntryPage.addField()
            }
        ]
    }

    Connections {
        target: keepassrx

        function onSavingChanged() {
            if (keepassrx.saving) {
                openSavingDialog();
            } else {
                closeSavingDialog();
            }
        }

        function onEntrySaved(entryUuid) {
            closeSavingDialog();
            if (saveRequested && entryUuid === entry.uuid) {
                saveRequested = false;
                pageStack.removePages(editEntryPage);
            }
        }

        function onEntryEditInvalid(problems) {
            closeSavingDialog();
            if (!saveRequested) {
                return;
            }

            saveRequested = false;
            showProblems(problems);
        }

        function onEntrySaveAuthRejected(left) {
            closeSavingDialog();
            if (!saveRequested) {
                return;
            }

            saveRequested = false;
            if (left > 0) {
                askForCode(true, left);
            }
        }

        function onEntrySaveFailed(message, conflict) {
            closeSavingDialog();
            if (!saveRequested) {
                return;
            }

            saveRequested = false;
            PopupUtils.open(failureDialog, editEntryPage, {
                message: message,
                conflict: conflict
            });
        }
    }

    ListModel {
        id: editableFieldsModel
    }

    ListModel {
        id: readOnlyFieldsModel
    }

    KeyboardFlickable {
        id: formFlickable
        anchors.top: editEntryPage.header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        contentWidth: width
        contentHeight: formColumn.height + units.gu(4)
        boundsMovement: Flickable.StopAtBounds
        clip: true
        onContentHeightChanged: {
            if (newFieldIndex >= 0) {
                showNewField();
            }
        }

        Column {
            id: formColumn
            x: units.gu(2)
            y: units.gu(2)
            width: parent.width - units.gu(4)
            spacing: units.gu(1)

            Label {
                text: i18n.tr("Title")
            }

            TextField {
                id: titleField
                width: parent.width
                text: entry.title
            }

            Label {
                text: i18n.tr("Username")
            }

            TextField {
                id: usernameField
                width: parent.width
                text: entry.username
                inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
            }

            Label {
                text: i18n.tr("Password")
            }

            TextField {
                id: passwordField
                width: parent.width
                text: entry.password
                echoMode: passwordShown ? TextInput.Normal : TextInput.Password
                inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText | Qt.ImhSensitiveData

                secondaryItem: AbstractButton {
                    height: passwordField.height
                    width: height
                    onClicked: passwordShown = !passwordShown

                    Icon {
                        anchors.centerIn: parent
                        width: units.gu(2.5)
                        height: width
                        name: passwordShown ? "view-off" : "view-on"
                    }
                }
            }

            Label {
                text: i18n.tr("URL")
            }

            TextField {
                id: urlField
                width: parent.width
                text: entry.url
                inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoPredictiveText
            }

            NotesListItem {
                x: -formColumn.x
                width: formFlickable.width
                notesText: editEntryPage.notesText
                modified: editEntryPage.notesText !== entry.notes
                onClicked: pageStack.addPageToNextColumn(
                    editEntryPage,
                    Qt.resolvedUrl("EditNotesPage.qml"),
                    { editor: editEntryPage }
                )
            }

            Label {
                text: i18n.tr("Other Fields")
                textSize: Label.Large
                visible: editableFieldsModel.count > 0 || readOnlyFieldsModel.count > 0
            }

            StyledItem {
                width: parent.width
                height: fieldsColumn.height
                visible: editableFieldsModel.count > 0
                activeFocusOnPress: false

                Column {
                    id: fieldsColumn
                    width: parent.width
                    spacing: units.gu(1)

                    Repeater {
                        id: fieldRepeater
                        model: editableFieldsModel

                        ListItem {
                            id: fieldRow
                            readonly property int fieldIndex: index
                            width: parent.width
                            height: fieldColumn.height
                            divider.visible: false
                            highlightColor: "transparent"
                            leadingActions: ListItemActions {
                                actions: [
                                    Action {
                                        iconName: "delete"
                                        text: i18n.tr("Delete")
                                        onTriggered: removeField(fieldRow.fieldIndex)
                                    }
                                ]
                            }

                            function focusName() {
                                nameInput.forceActiveFocus();
                            }

                            Column {
                                id: fieldColumn
                                width: parent.width
                                spacing: units.gu(1)

                                Column {
                                    width: parent.width

                                    RowLayout {
                                        width: parent.width
                                        height: units.gu(4)

                                        Label {
                                            Layout.fillWidth: true
                                            text: i18n.tr("Custom Field %1").arg(index + 1)
                                            elide: Text.ElideRight
                                        }

                                        AbstractButton {
                                            implicitWidth: protectedLabel.implicitWidth
                                            Layout.fillHeight: true
                                            onClicked: protectedBox.trigger()

                                            Label {
                                                id: protectedLabel
                                                anchors.verticalCenter: parent.verticalCenter
                                                text: i18n.tr("Protected")
                                            }
                                        }

                                        CheckBox {
                                            id: protectedBox
                                            checked: fieldProtected
                                            onCheckedChanged: fieldProtected = checked
                                        }
                                    }

                                    ListItems.ThinDivider {}
                                }

                                TextField {
                                    id: nameInput
                                    width: parent.width
                                    placeholderText: i18n.tr("Field name")
                                    text: fieldName
                                    errorHighlight: nameError !== ""
                                    onTextChanged: {
                                        fieldName = text;
                                        nameError = "";
                                    }
                                }

                                Label {
                                    width: parent.width
                                    visible: nameError !== ""
                                    text: nameError
                                    color: theme.palette.normal.negative
                                    wrapMode: Text.WordWrap
                                }

                                TextField {
                                    id: valueInput
                                    width: parent.width
                                    placeholderText: i18n.tr("Field value")
                                    text: fieldValue
                                    echoMode: fieldProtected && !valueShown ? TextInput.Password : TextInput.Normal
                                    inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText | Qt.ImhSensitiveData
                                    onTextChanged: fieldValue = text

                                    secondaryItem: AbstractButton {
                                        visible: fieldProtected
                                        height: valueInput.height
                                        width: fieldProtected ? height : 0
                                        onClicked: valueShown = !valueShown

                                        Icon {
                                            anchors.centerIn: parent
                                            width: units.gu(2.5)
                                            height: width
                                            name: valueShown ? "view-off" : "view-on"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Repeater {
                model: readOnlyFieldsModel

                Column {
                    width: parent.width

                    Label {
                        text: fieldName
                        color: theme.palette.normal.backgroundSecondaryText
                    }

                    Label {
                        width: parent.width
                        text: fieldValue
                        wrapMode: Text.WordWrap
                    }
                }
            }
        }
    }

    Component {
        id: savingDialogComponent

        SavingDialog {}
    }

    Component {
        id: codeDialog

        SaveCodeDialog {
            entryTitle: entry.title
            onConfirmed: editEntryPage.submit(code)
        }
    }

    Component {
        id: failureDialog

        SaveFailureDialog {}
    }
}
