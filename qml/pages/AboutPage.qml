import QtQuick 2.0
import Lomiri.Components 1.3 as UITK
import "../components"
import Qt.labs.settings 1.0

UITK.Page {
    property bool isARMv7: false
    header: UITK.PageHeader {
        id: header
        title: i18n.ctr("page header", "About")
    }

    Flickable {
	anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.topMargin: units.gu(2)
	anchors.leftMargin: units.gu(2)
	anchors.rightMargin: units.gu(2)
	contentWidth: width

	Column {
	    spacing: units.gu(2)

	    UITK.Label {
		anchors.fill: parent
		text: "This is the about page."
	    }
	}
    }
}
