import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// "label: value" line; the value can be selected and copied.
RowLayout {
    property alias label: caption.text
    property alias value: content.text
    property real labelWidth: Kirigami.Units.gridUnit * 8

    spacing: Kirigami.Units.largeSpacing

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
