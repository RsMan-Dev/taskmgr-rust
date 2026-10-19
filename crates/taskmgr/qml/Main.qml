import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.taskmgr

QQC2.ApplicationWindow {
    id: window

    width: Kirigami.Units.gridUnit * 56
    height: Kirigami.Units.gridUnit * 38
    minimumWidth: Kirigami.Units.gridUnit * 36
    minimumHeight: Kirigami.Units.gridUnit * 24
    visible: true
    title: "Gestionnaire des tâches"

    readonly property var snapshot: monitor.snapshot !== "" ? JSON.parse(monitor.snapshot) : null
    readonly property var systemInfo: monitor.info !== "" ? JSON.parse(monitor.info) : null
    readonly property var processes: monitor.processes !== "" ? JSON.parse(monitor.processes) : null

    property int currentPage: 0
    property string initialSelection: ""
    property bool sidebarCollapsed: false

    SystemMonitor {
        id: monitor
        Component.onCompleted: start()
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // Navigation, like the Windows rail: icons with labels, collapsible.
        ColumnLayout {
            Layout.fillHeight: true
            // Children fill the width, which would make the column grow too.
            Layout.fillWidth: false
            Layout.preferredWidth: window.sidebarCollapsed ? Kirigami.Units.gridUnit * 2.75 : Kirigami.Units.gridUnit * 11
            Layout.margins: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.smallSpacing

            Behavior on Layout.preferredWidth {
                NumberAnimation { duration: Kirigami.Units.shortDuration; easing.type: Easing.InOutQuad }
            }

            QQC2.ToolButton {
                icon.name: "application-menu"
                display: QQC2.AbstractButton.IconOnly
                text: window.sidebarCollapsed ? "Déplier le menu" : "Replier le menu"
                onClicked: window.sidebarCollapsed = !window.sidebarCollapsed
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }

            Repeater {
                model: [
                    { text: "Processus", icon: "view-process-tree" },
                    { text: "Performances", icon: "office-chart-area" },
                    { text: "Système", icon: "computer" },
                ]

                QQC2.ItemDelegate {
                    required property var modelData
                    required property int index
                    Layout.fillWidth: true
                    text: modelData.text
                    icon.name: modelData.icon
                    display: window.sidebarCollapsed ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                    highlighted: window.currentPage === index
                    onClicked: window.currentPage = index
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: window.sidebarCollapsed && hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                }
            }

            Item { Layout.fillHeight: true }
        }

        Kirigami.Separator {
            Layout.fillHeight: true
        }

        // Content on the "view" background, lighter than the window chrome.
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Kirigami.Theme.colorSet: Kirigami.Theme.View
            Kirigami.Theme.inherit: false
            color: Kirigami.Theme.backgroundColor

            StackLayout {
                anchors.fill: parent
                currentIndex: window.currentPage

                ProcessesPage {
                    groups: window.processes
                    snapshot: window.snapshot
                    monitor: monitor
                }
                PerformancePage {
                    snapshot: window.snapshot
                    monitor: monitor
                    Component.onCompleted: {
                        const [key, option] = window.initialSelection.split("+")
                        if (key) {
                            selectedKey = key
                        }
                        logicalProcessors = option === "logical"
                    }
                }
                SystemPage {
                    info: window.systemInfo
                    snapshot: window.snapshot
                }
            }
        }
    }
}
