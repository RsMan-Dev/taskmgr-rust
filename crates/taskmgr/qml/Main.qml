import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.taskmgr

QQC2.ApplicationWindow {
    id: window

    width: Kirigami.Units.gridUnit * 56
    height: Kirigami.Units.gridUnit * 38
    minimumWidth: Kirigami.Units.gridUnit * (mini ? 14 : 36)
    minimumHeight: Kirigami.Units.gridUnit * (mini ? 12 : 24)
    visible: true
    title: qsTr("Task Manager")

    readonly property var snapshot: monitor.snapshot !== "" ? JSON.parse(monitor.snapshot) : null
    readonly property var systemInfo: monitor.info !== "" ? JSON.parse(monitor.info) : null
    readonly property var processes: monitor.processes !== "" ? JSON.parse(monitor.processes) : null

    readonly property var pages: [
        { text: qsTr("Processes"), icon: "nav-processes" },
        { text: qsTr("Performance"), icon: "nav-performance" },
        { text: qsTr("Details"), icon: "nav-details" },
        { text: qsTr("Startup apps"), icon: "nav-startup" },
        { text: qsTr("Users"), icon: "nav-users" },
        { text: qsTr("Services"), icon: "nav-services" },
        { text: qsTr("System"), icon: "nav-system" },
        { text: qsTr("Settings"), icon: "nav-settings" },
    ]
    property int currentPage: Prefs.startPage
    property string initialSelection: ""
    property bool sidebarFolded: false

    // Mini mode: a small readout, optionally kept above other windows; F12
    // shows it full screen, Escape comes back.
    property bool mini: false
    property bool fullScreenMini: false
    property bool miniBeforeFullScreen: false
    property size normalSize: Qt.size(width, height)

    function enterMini() {
        if (mini) {
            return
        }
        normalSize = Qt.size(width, height)
        mini = true
        width = Kirigami.Units.gridUnit * 20
        height = Kirigami.Units.gridUnit * 24
        applyPin()
    }

    function leaveMini() {
        mini = false
        applyPin()
        width = normalSize.width
        height = normalSize.height
    }

    function applyPin() {
        const above = mini && Prefs.miniPinned && !fullScreenMini
        monitor.setKeepAbove(above)
        // X11 honours the hint; on Wayland the KWin script above does it.
        if (Qt.platform.pluginName === "xcb") {
            flags = above ? (flags | Qt.WindowStaysOnTopHint) : (flags & ~Qt.WindowStaysOnTopHint)
        }
    }

    Shortcut {
        sequence: "F12"
        onActivated: {
            if (window.fullScreenMini) {
                return
            }
            window.miniBeforeFullScreen = window.mini
            window.enterMini()
            window.fullScreenMini = true
            window.applyPin()
            window.showFullScreen()
        }
    }
    Shortcut {
        sequence: "Escape"
        enabled: window.fullScreenMini
        onActivated: {
            window.showNormal()
            window.fullScreenMini = false
            if (!window.miniBeforeFullScreen) {
                window.leaveMini()
            } else {
                window.applyPin()
            }
        }
    }
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
    readonly property bool processesShown: mini || [0, 2, 4].includes(currentPage)
    onProcessesShownChanged: monitor.setProcessesVisible(processesShown)
    Component.onCompleted: {
        monitor.setProcessesVisible(processesShown)
        if (initialSelection === "mini") {
            enterMini()
        }
        details.tree = initialSelection === "tree"
    }

    component NavButton: QQC2.ItemDelegate {
        required property var modelData
        required property int index
        Layout.fillWidth: true
        text: modelData.text
        icon.source: Icons.of(modelData.icon)
        icon.color: highlighted ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
        display: window.sidebarCollapsed ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
        highlighted: window.currentPage === index
        onClicked: window.currentPage = index
        QQC2.ToolTip.text: text
        QQC2.ToolTip.visible: window.sidebarCollapsed && hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
    }

    MiniView {
        visible: window.mini
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * scale
        snapshot: window.snapshot
        groups: window.processes
        pinned: Prefs.miniPinned
        scale: window.fullScreenMini ? 2 : 1
        onExpandRequested: {
            if (window.fullScreenMini) {
                window.showNormal()
                window.fullScreenMini = false
            }
            window.leaveMini()
        }
        onPinToggled: {
            Prefs.miniPinned = !Prefs.miniPinned
            window.applyPin()
        }
    }

    RowLayout {
        visible: !window.mini
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
                icon.source: Icons.of("menu")
                icon.color: Kirigami.Theme.textColor
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

            QQC2.ItemDelegate {
                Layout.fillWidth: true
                text: qsTr("Mini mode")
                icon.source: Icons.of("minimize")
                icon.color: Kirigami.Theme.textColor
                display: window.sidebarCollapsed ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                onClicked: window.enterMini()
                QQC2.ToolTip.text: qsTr("Mini mode (F12: full screen)")
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }

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
                        cpuView = option || "overall"
                    }
                }
                DetailsPage {
                    id: details
                    groups: window.processes
                    monitor: monitor
                    logical: window.snapshot?.cpu.logical ?? 1
                }
                StartupPage {
                    monitor: monitor
                    active: window.currentPage === 3
                }
                UsersPage {
                    groups: window.processes
                    logical: window.snapshot?.cpu.logical ?? 1
                    onShowUser: (user) => {
                        details.query = user
                        window.currentPage = 2
                    }
                }
                ServicesPage {
                    monitor: monitor
                    active: window.currentPage === 5
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
