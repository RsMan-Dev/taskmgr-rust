import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// A progress bar with Breeze's look. Breeze's own ProgressBar logs errors
// whenever it is resized (its indeterminate stripes lose their parent).
Rectangle {
    id: line

    property real value: 0

    implicitHeight: 6
    radius: height / 2
    color: Kirigami.Theme.alternateBackgroundColor
    border.width: 1
    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
    Layout.fillWidth: true

    Rectangle {
        width: Math.max(line.height, line.width * Math.min(1, Math.max(0, line.value)))
        height: line.height
        radius: line.radius
        color: Kirigami.Theme.highlightColor
        visible: line.value > 0
    }
}
