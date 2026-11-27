import QtQuick 2.6
import Sailfish.Silica 1.0
import keepassrx 1.0

import "../components"

Dialog {
    id: editPage
    allowedOrientations: defaultAllowedOrientations
    acceptDestination: Qt.resolvedUrl("SavingPage.qml")
    acceptDestinationProperties: { "editor": editPage }

    property var entry: null
    property var readOnlyFields: []
    property bool saveRequested: false
    property bool draftSaved: false
    property bool cleared: false
    property string titleText: entry ? entry.title : ""
    property string usernameText: entry ? entry.username : ""
    property string passwordText: entry ? entry.password : ""
    property string urlText: entry ? entry.url : ""
    property string notesText: entry ? entry.notes : ""
    property int pendingRow: -1
    property var savingPage: null
    property bool codeRejected: false
    property bool codeFallback: false
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
            title: titleText,
            username: usernameText,
            password: passwordText,
            url: urlText,
            notes: notesText,
            customFields: custom,
            removedFields: removed
        };
    }

    function clearValues() {
        cleared = true;
        titleText = "";
        usernameText = "";
        passwordText = "";
        urlText = "";
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
        if (!customSection.expanded) {
            customSection.expanded = true;
            pendingRow = index;
            scrollTimer.restart();
            return;
        }
        var fields = customSection.content.item;
        var row = fields ? fields.rowAt(index) : null;
        if (!row) {
            return;
        }

        var top = row.mapToItem(column, 0, 0).y - Theme.paddingLarge;
        var maxY = Math.max(0, flick.contentHeight - flick.height);
        flick.contentY = Math.max(0, Math.min(top, maxY));
    }

    Timer {
        id: codeTimer
        interval: 50
        onTriggered: editPage.askForCode(editPage.codeRejected, editPage.codeFallback)
    }

    Timer {
        id: scrollTimer
        interval: 0
        onTriggered: {
            var index = editPage.pendingRow;
            editPage.pendingRow = -1;
            editPage.scrollToRow(index);
        }
    }

    function beginSave(page) {
        savingPage = page;
        var problems = keepassrx.validateEntryEdit(collectValues());
        if (problems.length > 0) {
            savingPage.finish("back");
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
        if (pageStack.busy) {
            codeRejected = rejected;
            codeFallback = fallback === true;
            codeTimer.restart();
            return;
        }
        var dialog = pageStack.push(Qt.resolvedUrl("SaveCodeDialog.qml"), {
            rejected: rejected,
            fallback: fallback === true,
            entryTitle: entry.title
        });
        dialog.accepted.connect(function () {
            submit(dialog.password);
        });
        dialog.rejected.connect(function () {
            if (savingPage) {
                savingPage.finish("back");
            }
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
            if (entryUuid !== editPage.entry.uuid || !editPage.saveRequested) {
                return;
            }
            editPage.saveRequested = false;
            if (editPage.savingPage) {
                editPage.savingPage.finish("saved");
            } else if (editPage.status === PageStatus.Active) {
                pageStack.pop(pageStack.previousPage(editPage));
            }
        }

        onEntryEditInvalid: {
            editPage.saveRequested = false;
            if (editPage.savingPage) {
                editPage.savingPage.finish("back");
            }
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
            if (editPage.savingPage) {
                editPage.savingPage.finish("back");
            }
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

        Column {
            id: column
            width: parent.width

            DialogHeader {
                title: Tr.tr("Edit entry")
                acceptText: Tr.tr("Save")
                cancelText: Tr.tr("Cancel")
            }

            ExpandingSection {
                id: mainSection
                title: Tr.tr("Main")
                expanded: true

                content.sourceComponent: Column {
                    width: mainSection.width

                    TextField {
                        id: titleField
                        width: parent.width
                        label: Tr.tr("Title")
                        placeholderText: Tr.tr("Title")
                        text: editPage.titleText
                        onTextChanged: editPage.titleText = text
                        EnterKey.iconSource: "image://theme/icon-m-enter-next"
                        EnterKey.onClicked: usernameField.focus = true
                    }

                    TextField {
                        id: usernameField
                        width: parent.width
                        label: Tr.tr("Username")
                        placeholderText: Tr.tr("Username")
                        text: editPage.usernameText
                        onTextChanged: editPage.usernameText = text
                        inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                        EnterKey.iconSource: "image://theme/icon-m-enter-next"
                        EnterKey.onClicked: passwordField.focus = true
                    }

                    PasswordField {
                        id: passwordField
                        width: parent.width
                        label: Tr.tr("Password")
                        placeholderText: Tr.tr("Password")
                        text: editPage.passwordText
                        onTextChanged: editPage.passwordText = text
                        EnterKey.iconSource: "image://theme/icon-m-enter-next"
                        EnterKey.onClicked: urlField.focus = true
                    }

                    TextField {
                        id: urlField
                        width: parent.width
                        label: Tr.tr("URL")
                        placeholderText: Tr.tr("URL")
                        text: editPage.urlText
                        onTextChanged: editPage.urlText = text
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
                }
            }

            ExpandingSection {
                id: customSection
                title: Tr.tr("Custom fields")

                content.sourceComponent: Column {
                    width: customSection.width

                    function rowAt(index) {
                        return fieldRepeater.itemAt(index);
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

                                SectionHeader {
                                    text: fieldName.length === 0
                                        ? Tr.tr("New field")
                                        : originalName.length > 0 && originalName !== fieldName
                                            ? Tr.tr("%1 (renamed from %2)").arg(fieldName).arg(originalName)
                                            : fieldName
                                }

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

                                Loader {
                                    width: parent.width
                                    sourceComponent: isProtected ? protectedValueField : plainValueField
                                }

                                Component {
                                    id: plainValueField
                                    TextField {
                                        width: parent.width
                                        label: Tr.tr("Field value")
                                        placeholderText: Tr.tr("Field value")
                                        inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText | Qt.ImhSensitiveData
                                        Component.onCompleted: text = fieldValue
                                        onTextChanged: fieldsModel.setProperty(index, "fieldValue", text)
                                    }
                                }

                                Component {
                                    id: protectedValueField
                                    PasswordField {
                                        width: parent.width
                                        label: Tr.tr("Field value")
                                        placeholderText: Tr.tr("Field value")
                                        inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText | Qt.ImhSensitiveData
                                        Component.onCompleted: text = fieldValue
                                        onTextChanged: fieldsModel.setProperty(index, "fieldValue", text)
                                    }
                                }

                                TextSwitch {
                                    width: parent.width
                                    text: Tr.tr("Protected")
                                    checked: isProtected
                                    onCheckedChanged: fieldsModel.setProperty(index, "isProtected", checked)
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
                }
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
