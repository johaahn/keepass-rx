import QtQuick 2.6
import Sailfish.Silica 1.0
import keepassrx 1.0

import "../components"

Page {
    id: editPage
    allowedOrientations: defaultAllowedOrientations

    property var entry: null
    property var readOnlyFields: []
    property bool saveRequested: false
    property bool draftSaved: false
    property bool cleared: false
    property string notesText: entry ? entry.notes : ""
    readonly property bool notesModified: entry !== null && notesText !== entry.notes

    function loadFields() {
        if (!entry || !entry.customFields) {
            return;
        }
        var fields = [];
        for (var i = 0; i < entry.customFields.length; i++) {
            var f = entry.customFields[i];
            if (f.editable) {
                fieldsModel.append({
                    originalName: f.originalName !== undefined ? f.originalName : f.name,
                    fieldName: f.name,
                    fieldValue: f.value ? f.value : "",
                    isProtected: f.protected === true,
                    nameError: ""
                });
            } else {
                fields.push(f);
            }
        }
        readOnlyFields = fields;
        var removed = entry.removedFields ? entry.removedFields : [];
        for (var j = 0; j < removed.length; j++) {
            removedModel.append({ fieldName: removed[j] });
        }
    }

    function collectValues() {
        var custom = [];
        for (var i = 0; i < fieldsModel.count; i++) {
            var f = fieldsModel.get(i);
            custom.push({
                originalName: f.originalName,
                name: f.fieldName,
                value: f.fieldValue,
                protected: f.isProtected
            });
        }
        var removed = [];
        for (var j = 0; j < removedModel.count; j++) {
            removed.push(removedModel.get(j).fieldName);
        }
        return {
            title: titleField.text,
            username: usernameField.text,
            password: passwordField.text,
            url: urlField.text,
            notes: notesText,
            customFields: custom,
            removedFields: removed
        };
    }

    function clearValues() {
        cleared = true;
        titleField.text = "";
        usernameField.text = "";
        passwordField.text = "";
        urlField.text = "";
        notesText = "";
        fieldsModel.clear();
        removedModel.clear();
        readOnlyFields = [];
        entry = { uuid: entry.uuid };
    }

    function showProblems(problems) {
        for (var i = 0; i < fieldsModel.count; i++) {
            fieldsModel.setProperty(i, "nameError", "");
        }

        var first = -1;
        for (var j = 0; j < problems.length; j++) {
            fieldsModel.setProperty(problems[j].index, "nameError", problems[j].message);
            if (first === -1 || problems[j].index < first) {
                first = problems[j].index;
            }
        }

        if (first !== -1) {
            scrollToRow(first);
        }
    }

    function scrollToRow(index) {
        var row = fieldRepeater.itemAt(index);
        if (!row) {
            return;
        }

        var top = row.mapToItem(column, 0, 0).y - Theme.paddingLarge;
        var maxY = Math.max(0, flick.contentHeight - flick.height);
        flick.contentY = Math.max(0, Math.min(top, maxY));
    }

    function save() {
        focus = true;
        var problems = keepassrx.validateEntryEdit(collectValues());
        if (problems.length > 0) {
            showProblems(problems);
            return;
        }

        if (keepassrx.saveAuthMethod === "system") {
            submit("");
        } else {
            askForCode(false);
        }
    }

    function askForCode(rejected, fallback) {
        var dialog = pageStack.push(Qt.resolvedUrl("SaveCodeDialog.qml"), {
            rejected: rejected,
            fallback: fallback === true,
            entryTitle: entry.title
        });
        dialog.accepted.connect(function () {
            submit(dialog.password);
        });
    }

    function saveDraft() {
        if (cleared) {
            return;
        }
        draftSaved = true;
        keepassrx.saveDraft(entry.uuid, collectValues());
    }

    function submit(credential) {
        if (cleared) {
            return;
        }
        saveRequested = true;
        keepassrx.saveEntry(entry.uuid, collectValues(), credential);
    }

    Component.onCompleted: {
        loadFields();
        applicationWindow.entryEditor = editPage;
    }

    Component.onDestruction: {
        if (applicationWindow.entryEditor === editPage) {
            applicationWindow.entryEditor = null;
        }
        if (!draftSaved && entry) {
            keepassrx.discardDraft(entry.uuid);
        }
    }

    ListModel { id: fieldsModel }
    ListModel { id: removedModel }

    Connections {
        target: keepassrx

        onEntrySaved: {
            if (entryUuid === editPage.entry.uuid) {
                pageStack.pop();
            }
        }

        onEntryEditInvalid: {
            editPage.saveRequested = false;
            editPage.showProblems(problems);
        }

        onEntrySaveAuthRejected: {
            if (editPage.saveRequested) {
                editPage.saveRequested = false;
                editPage.askForCode(true);
            }
        }

        onEntrySavePasswordRejected: {
            if (editPage.saveRequested) {
                editPage.saveRequested = false;
                editPage.askForCode(true);
            }
        }

        onEntrySaveCodeRequired: {
            if (editPage.saveRequested) {
                editPage.saveRequested = false;
                editPage.askForCode(false, true);
            }
        }

        onEntrySaveFailed: {
            editPage.saveRequested = false;
            Notices.show(conflict
                ? Tr.tr("The database file changed on disk. Close and reopen the database, then edit again.")
                : message, Notice.Long);
        }
    }

    SilicaFlickable {
        id: flick
        anchors.fill: parent
        contentHeight: column.height + Theme.paddingLarge

        VerticalScrollDecorator {}

        PullDownMenu {
            MenuItem {
                text: Tr.tr("Save")
                onClicked: editPage.save()
            }
        }

        Column {
            id: column
            width: parent.width

            PageHeader {
                title: Tr.tr("Edit entry")
            }

            TextField {
                id: titleField
                width: parent.width
                label: Tr.tr("Title")
                placeholderText: Tr.tr("Title")
                text: entry ? entry.title : ""
                EnterKey.iconSource: "image://theme/icon-m-enter-next"
                EnterKey.onClicked: usernameField.focus = true
            }

            TextField {
                id: usernameField
                width: parent.width
                label: Tr.tr("Username")
                placeholderText: Tr.tr("Username")
                text: entry ? entry.username : ""
                inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                EnterKey.iconSource: "image://theme/icon-m-enter-next"
                EnterKey.onClicked: passwordField.focus = true
            }

            PasswordField {
                id: passwordField
                width: parent.width
                label: Tr.tr("Password")
                placeholderText: Tr.tr("Password")
                text: entry ? entry.password : ""
                EnterKey.iconSource: "image://theme/icon-m-enter-next"
                EnterKey.onClicked: urlField.focus = true
            }

            TextField {
                id: urlField
                width: parent.width
                label: Tr.tr("URL")
                placeholderText: Tr.tr("URL")
                text: entry ? entry.url : ""
                inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoPredictiveText
                EnterKey.iconSource: "image://theme/icon-m-enter-close"
                EnterKey.onClicked: focus = false
            }

            NotesSummaryItem {
                width: parent.width
                notesText: editPage.notesText
                modified: editPage.notesModified
                onClicked: pageStack.push(Qt.resolvedUrl("EditNotesPage.qml"), { editor: editPage })
            }

            SectionHeader {
                text: Tr.tr("Other fields")
                visible: fieldsModel.count > 0
            }

            Repeater {
                id: fieldRepeater
                model: fieldsModel

                ListItem {
                    id: fieldItem
                    contentHeight: fieldColumn.height

                    function deleteField() {
                        var original = originalName;
                        var position = index;
                        fieldItem.remorseDelete(function () {
                            if (original.length > 0) {
                                removedModel.append({ fieldName: original });
                            }
                            fieldsModel.remove(position);
                        });
                    }

                    menu: ContextMenu {
                        MenuItem {
                            text: Tr.tr("Delete field")
                            onClicked: fieldItem.deleteField()
                        }
                    }

                    Column {
                        id: fieldColumn
                        width: parent.width

                        TextField {
                            width: parent.width
                            label: Tr.tr("Field name")
                            placeholderText: Tr.tr("Field name")
                            errorHighlight: nameError !== ""
                            description: nameError
                            Component.onCompleted: text = fieldName
                            onTextChanged: {
                                fieldsModel.setProperty(index, "fieldName", text);
                                fieldsModel.setProperty(index, "nameError", "");
                            }
                        }

                        PasswordField {
                            width: parent.width
                            label: Tr.tr("Field value")
                            placeholderText: Tr.tr("Field value")
                            showEchoModeToggle: isProtected
                            passwordEchoMode: isProtected ? TextInput.Password : TextInput.Normal
                            inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText | Qt.ImhSensitiveData
                            Component.onCompleted: text = fieldValue
                            onTextChanged: fieldsModel.setProperty(index, "fieldValue", text)
                        }

                        Item {
                            width: parent.width
                            height: protectedSwitch.height

                            TextSwitch {
                                id: protectedSwitch
                                anchors.left: parent.left
                                anchors.right: deleteButton.left
                                text: Tr.tr("Protected")
                                checked: isProtected
                                onCheckedChanged: fieldsModel.setProperty(index, "isProtected", checked)
                            }

                            IconButton {
                                id: deleteButton
                                anchors.right: parent.right
                                anchors.rightMargin: Theme.horizontalPageMargin
                                anchors.verticalCenter: parent.verticalCenter
                                icon.source: "image://theme/icon-m-delete"
                                onClicked: fieldItem.deleteField()
                            }
                        }
                    }
                }
            }

            Button {
                anchors.horizontalCenter: parent.horizontalCenter
                text: Tr.tr("Add field")
                onClicked: fieldsModel.append({
                    originalName: "",
                    fieldName: "",
                    fieldValue: "",
                    isProtected: false,
                    nameError: ""
                })
            }

            SectionHeader {
                text: Tr.tr("Read-only fields")
                visible: editPage.readOnlyFields.length > 0
            }

            Repeater {
                model: editPage.readOnlyFields

                DetailItem {
                    label: modelData.name
                    value: modelData.protected ? "••••••" : modelData.value
                }
            }
        }
    }

    SavingOverlay {
        anchors.fill: parent
    }
}
