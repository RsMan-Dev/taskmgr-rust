import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Navigation, like the Windows rail: icons with labels, collapsible. The
// last page (settings) sits at the bottom, under mini mode.
ColumnLayout {
    id: rail

    required property var pages
    required property int currentPage
    property bool collapsed: false

    signal pageRequested(int index)
    signal toggleRequested()
    signal miniRequested()

    Layout.fillHeight: true
    // Children fill the width, which would make the column grow too.
    Layout.fillWidth: false
    Layout.preferredWidth: collapsed ? Kirigami.Units.gridUnit * 2.75 : Kirigami.Units.gridUnit * 11
    Layout.margins: Kirigami.Units.smallSpacing
    spacing: Kirigami.Units.smallSpacing

    Behavior on Layout.preferredWidth {
        NumberAnimation { duration: Kirigami.Units.shortDuration; easing.type: Easing.InOutQuad }
    }

    QQC2.ToolButton {
        icon.source: Icons.of("menu", Kirigami.Theme.textColor)
        display: QQC2.AbstractButton.IconOnly
        text: rail.collapsed ? qsTr("Expand menu") : qsTr("Collapse menu")
        onClicked: rail.toggleRequested()
        QQC2.ToolTip.text: text
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
    }

    Repeater {
        model: rail.pages.slice(0, rail.pages.length - 1)
        NavButton {
            required property var modelData
            required property int index
            text: modelData.text
            iconName: modelData.icon
            current: rail.currentPage === index
            collapsed: rail.collapsed
            onClicked: rail.pageRequested(index)
        }
    }

    Item { Layout.fillHeight: true }

    NavButton {
        text: qsTr("Mini mode")
        iconName: "minimize"
        collapsed: rail.collapsed
        toolTip: qsTr("Mini mode (F12: full screen)")
        onClicked: rail.miniRequested()
    }

    NavButton {
        readonly property int index: rail.pages.length - 1
        text: rail.pages[index].text
        iconName: rail.pages[index].icon
        current: rail.currentPage === index
        collapsed: rail.collapsed
        onClicked: rail.pageRequested(index)
    }
}
