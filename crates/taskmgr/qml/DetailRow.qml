import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Static "label: value" line.
RowLayout {
    property alias label: caption.text
    property alias value: content.text
    property real labelWidth: Kirigami.Units.gridUnit * 8

    spacing: Kirigami.Units.largeSpacing

    QQC2.Label {
        id: caption
        opacity: 0.7
        Layout.preferredWidth: parent.labelWidth
        Layout.alignment: Qt.AlignTop
        elide: Text.ElideRight
    }
    QQC2.Label {
        id: content
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
    }
}
