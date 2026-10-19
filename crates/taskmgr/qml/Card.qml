import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Rectangle {
    Layout.fillWidth: true
    Layout.maximumWidth: Kirigami.Units.gridUnit * 48
    radius: Kirigami.Units.cornerRadius * 2
    color: Qt.alpha(Kirigami.Theme.textColor, 0.04)
    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.09)
}
