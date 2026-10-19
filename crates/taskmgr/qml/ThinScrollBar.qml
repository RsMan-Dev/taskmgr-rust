import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Slim scroll bar on an opaque track, so content never shows through it.
QQC2.ScrollBar {
    id: bar

    readonly property real thickness: Math.round(Kirigami.Units.gridUnit * 0.45)

    minimumSize: 0.05
    padding: 1
    visible: policy === QQC2.ScrollBar.AlwaysOn || (policy === QQC2.ScrollBar.AsNeeded && size < 1)
    implicitWidth: orientation === Qt.Vertical ? thickness : 0
    implicitHeight: orientation === Qt.Horizontal ? thickness : 0

    background: Rectangle {
        color: Kirigami.Theme.backgroundColor
    }

    contentItem: Rectangle {
        implicitWidth: bar.thickness - 2
        implicitHeight: bar.thickness - 2
        radius: Math.min(width, height) / 2
        color: Kirigami.Theme.textColor
        opacity: bar.pressed ? 0.6 : bar.hovered ? 0.45 : 0.28
    }
}
