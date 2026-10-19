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
    title: qsTr("Task Manager")

    readonly property var snapshot: monitor.snapshot !== "" ? JSON.parse(monitor.snapshot) : null
    readonly property var systemInfo: monitor.info !== "" ? JSON.parse(monitor.info) : null
    readonly property var processes: monitor.processes !== "" ? JSON.parse(monitor.processes) : null

    readonly property var pages: [
        { text: qsTr("Processes"), icon: "view-process-tree" },
        { text: qsTr("Performance"), icon: "office-chart-area" },
        { text: qsTr("Details"), icon: "view-list-details" },
        { text: qsTr("Startup apps"), icon: "system-run" },
        { text: qsTr("Services"), icon: "preferences-system-services" },
        { text: qsTr("System"), icon: "computer" },
        { text: qsTr("Settings"), icon: "configure" },
    ]
    property int currentPage: Prefs.startPage
    property string initialSelection: ""
    property bool sidebarFolded: false
    property bool sidebarOpenedNarrow: false
    readonly property bool narrow: width < Kirigami.Units.gridUnit * 46
    readonly property bool sidebarCollapsed: narrow ? !sidebarOpenedNarrow : sidebarFolded

    SystemMonitor {
        id: monitor
        Component.onCompleted: {
            setInterval(Prefs.intervalMs)
            start()
        }
    }

    Connections {
        target: Prefs
        function onIntervalMsChanged() {
            monitor.setInterval(Prefs.intervalMs)
        }
    }

    // The process list is only sampled while a page shows it.
    onCurrentPageChanged: monitor.setProcessesVisible(currentPage === 0 || currentPage === 2)
    Component.onCompleted: monitor.setProcessesVisible(currentPage === 0 || currentPage === 2)

    component NavButton: QQC2.ItemDelegate {
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
                text: window.sidebarCollapsed ? qsTr("Expand menu") : qsTr("Collapse menu")
                onClicked: {
                    if (window.narrow) {
                        window.sidebarOpenedNarrow = !window.sidebarOpenedNarrow
                    } else {
                        window.sidebarFolded = !window.sidebarFolded
                    }
                }
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }

            Repeater {
                model: window.pages.slice(0, window.pages.length - 1)
                NavButton {}
            }

            Item { Layout.fillHeight: true }

            NavButton {
                modelData: window.pages[window.pages.length - 1]
                index: window.pages.length - 1
            }
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
                DetailsPage {
                    groups: window.processes
                    monitor: monitor
                    logical: window.snapshot?.cpu.logical ?? 1
                }
                StartupPage {
                    monitor: monitor
                    active: window.currentPage === 3
                }
                ServicesPage {
                    monitor: monitor
                    active: window.currentPage === 4
                }
                SystemPage {
                    info: window.systemInfo
                    snapshot: window.snapshot
                }
                SettingsPage {
                    pageNames: window.pages.slice(0, window.pages.length - 1).map(p => p.text)
                }
            }
        }
    }
}
