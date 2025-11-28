import QtQuick 2.0
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3
import Qt.labs.settings 1.0
import QtWebView 1.15

Page {
    header: PageHeader {
        id: header
        // TRANSLATORS: List of 3rd party licenses used in this project.
        title: i18n.tr("Open Source Licenses")
    }

    WebView {
        id: webview
        url: "qrc:/webengine/assets/licenses.html"
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
    }
}
