import QtQuick 2.0
import QtQuick.Layouts 1.12
import Lomiri.Components 1.3 as UITK
import "../components"
import Qt.labs.settings 1.0

UITK.Page {
    ColumnLayout {
	spacing: units.gu(2)

	RowLayout {
	    Layout.fillWidth: true

	    Rectangle {
		height: units.gu(35)
		Layout.fillWidth: true
		Layout.alignment: Qt.AlignVCenter | Qt.AlignHCenter
	        color: "transparent"

		Image {
		    width: units.gu(35)
		    height: units.gu(35)
		    fillMode: Image.PreserveAspectFit
		    source: '../../assets/keepass-rx.svg'
		    x: parent.width / 2 - width / 2
		    y: parent.height / 2 - height / 2
		}
	    }
	}

	anchors {
	    margins: units.gu(2)
	    top: parent.top
	    left: parent.left
	    right: parent.right
	    bottom: parent.bottom
        }

	Item {
	    Layout.fillHeight: false
        }

	UITK.Label {
	    width: parent.width
	    Layout.preferredWidth: width
	    wrapMode: Text.Wrap
	    text: i18n.tr("Blank page thing")
	}

	Item {
	    Layout.fillHeight: true
        }
    }
}
