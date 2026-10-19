import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Live value with its caption above, as in the Windows performance pane.
ColumnLayout {
    property alias label: caption.text
    property alias value: number.text

    spacing: 0

    QQC2.Label {
        id: caption
        opacity: 0.7
        font: Kirigami.Theme.smallFont
    }
    QQC2.Label {
        id: number
        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.5
    }
}
