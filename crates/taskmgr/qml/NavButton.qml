import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Navigation entry. Selection as in Windows 11: a light fill and an accent bar.
QQC2.ItemDelegate {
    id: button

    property string iconName
    property bool current: false
    property bool collapsed: false
    property string toolTip: collapsed ? text : ""

    Layout.fillWidth: true
    icon.source: Icons.of(iconName, Kirigami.Theme.textColor)
    display: collapsed ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
    QQC2.ToolTip.text: toolTip
    QQC2.ToolTip.visible: toolTip !== "" && hovered
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

    background: Rectangle {
        readonly property color ink: Kirigami.Theme.textColor
        radius: Kirigami.Units.cornerRadius
        color: Qt.rgba(ink.r, ink.g, ink.b,
                       (button.current ? 0.07 : 0) + (button.down ? 0.05 : button.hovered ? 0.04 : 0))
        Rectangle {
            visible: button.current
            width: 3
            height: parent.height * 0.45
            radius: width / 2
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            color: Kirigami.Theme.highlightColor
        }
    }
}
