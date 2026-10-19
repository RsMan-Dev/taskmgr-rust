import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// "label: value" line, the label above the value when narrow; the value
// can be selected and copied.
GridLayout {
    property alias label: caption.text
    property alias value: content.text
    property real labelWidth: Kirigami.Units.gridUnit * 8

    columns: width >= labelWidth + Kirigami.Units.gridUnit * 9 ? 2 : 1
    columnSpacing: Kirigami.Units.largeSpacing
    rowSpacing: 0

    QQC2.Label {
        id: caption
        opacity: 0.7
        wrapMode: Text.Wrap
        Layout.preferredWidth: parent.labelWidth
        Layout.alignment: Qt.AlignTop
    }
    SelectableText {
        id: content
        wrapMode: TextEdit.Wrap
        Layout.fillWidth: true
    }
}
