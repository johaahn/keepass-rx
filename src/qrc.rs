qrc!(qml_resources,
     "/" {
         "assets/visibility.png",
         "assets/visibility_off.png",
         "assets/logo.png",
         "assets/keepass-rx.png",
         "assets/placeholder.png",
         "assets/user.svg",
         "assets/key.svg",
         "assets/2fa.svg",
         "qml/Main.qml",
         "qml/pages/EntriesPage.qml",
         "qml/pages/OpenDBPage.qml",
         "qml/pages/SettingsPage.qml",
         "qml/pages/AboutPage.qml",
         "qml/components/DBEntry.qml",
         "qml/components/SettingsItem.qml",
         "qml/components/TextFieldPlaceholder.qml",
    },
);

pub fn load() {
    qml_resources();
}
