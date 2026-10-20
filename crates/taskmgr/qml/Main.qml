import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.taskmgr

QQC2.ApplicationWindow {
    id: window

    width: Kirigami.Units.gridUnit * 56
    height: Kirigami.Units.gridUnit * 38
    minimumWidth: Kirigami.Units.gridUnit * (mini ? 14 : 20)
    minimumHeight: Kirigami.Units.gridUnit * (mini ? 12 : 22)
    visible: true
    title: qsTr("Task Manager")

    // Styles other than org.kde.desktop (Fusion in the AppImage) keep their
    // own palette: hand them the application one.
    SystemPalette { id: appPalette }
    SystemPalette { id: appPaletteOff; colorGroup: SystemPalette.Disabled }
    palette.window: appPalette.window
    palette.windowText: appPalette.windowText
    palette.base: appPalette.base
    palette.alternateBase: appPalette.alternateBase
    palette.text: appPalette.text
    palette.button: appPalette.button
    palette.buttonText: appPalette.buttonText
    palette.highlight: appPalette.highlight
    palette.highlightedText: appPalette.highlightedText
    palette.placeholderText: appPalette.placeholderText
    palette.mid: appPalette.mid
    palette.midlight: appPalette.midlight
    palette.dark: appPalette.dark
    palette.light: appPalette.light
    palette.shadow: appPalette.shadow
    palette.disabled.windowText: appPaletteOff.windowText
    palette.disabled.text: appPaletteOff.text
    palette.disabled.buttonText: appPaletteOff.buttonText

    readonly property var snapshot: monitor.snapshot !== "" ? JSON.parse(monitor.snapshot) : null
    readonly property var systemInfo: monitor.info !== "" ? JSON.parse(monitor.info) : null
    readonly property var processes: monitor.processes !== "" ? JSON.parse(monitor.processes) : null
    // pid -> { command, exe }, refreshed every few seconds.
    readonly property var processDetails: monitor.processDetails !== "" ? JSON.parse(monitor.processDetails) : ({})

    readonly property var pages: [
        { text: qsTr("Processes"), icon: "nav-processes" },
        { text: qsTr("Performance"), icon: "nav-performance" },
        { text: qsTr("Details"), icon: "nav-details" },
        { text: qsTr("Startup apps"), icon: "nav-startup" },
        { text: qsTr("Users"), icon: "nav-users" },
        { text: qsTr("Services"), icon: "nav-services" },
        { text: qsTr("Connections"), icon: "nav-connections" },
        { text: qsTr("Crashes"), icon: "nav-crashes" },
        { text: qsTr("Benchmark"), icon: "nav-benchmark" },
        { text: qsTr("System"), icon: "nav-system" },
        { text: qsTr("Settings"), icon: "nav-settings" },
    ]
    property int currentPage: Prefs.startPage
    property string initialSelection: ""
    readonly property double openedAt: Date.now()

    // Hidden pages get no data: they would recompute on every refresh.
    function shown(page, value) {
        return !mini && currentPage === page ? value : null
    }
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
            setHistoryPoints(Prefs.historyPoints)
            setVirtualNetworks(Prefs.showVirtualNetworks)
            start()
        }
    }

    Connections {
        target: Prefs
        function onIntervalMsChanged() {
            monitor.setInterval(Prefs.intervalMs)
        }
        function onShowVirtualNetworksChanged() {
            monitor.setVirtualNetworks(Prefs.showVirtualNetworks)
        }
        function onHistoryPointsChanged() {
            monitor.setHistoryPoints(Prefs.historyPoints)
        }
    }

    // The process list is only sampled while a page shows it.
    readonly property bool processesShown: mini || [0, 2, 4, 6].includes(currentPage)
    onProcessesShownChanged: monitor.setProcessesVisible(processesShown)
    Component.onCompleted: {
        monitor.setProcessesVisible(processesShown)
        if (initialSelection === "mini") {
            enterMini()
        }
        details.tree = initialSelection === "tree"
    }

    Shortcut {
        sequences: [StandardKey.Find]
        onActivated: stack.children[window.currentPage]?.focusSearch?.()
    }
    Repeater {
        model: 9
        Item {
            required property int index
            Shortcut {
                sequence: "Ctrl+" + (index + 1)
                onActivated: window.currentPage = index
            }
        }
    }
    Shortcut {
        sequence: "Ctrl+,"
        onActivated: window.currentPage = stack.children.length - 1
    }
    Shortcut {
        property int paused: 1000
        sequence: "Ctrl+P"
        onActivated: {
            if (Prefs.intervalMs > 0) {
                paused = Prefs.intervalMs
                Prefs.intervalMs = 0
            } else {
                Prefs.intervalMs = paused
            }
        }
    }
    Shortcut {
        sequences: [StandardKey.HelpContents]
        onActivated: shortcutsDialog.open()
    }
    Shortcut {
        sequences: [StandardKey.Quit]
        onActivated: Qt.quit()
    }

    ShortcutsDialog {
        id: shortcutsDialog
    }

    JournalDialog {
        id: journalDialog
        monitor: monitor
    }

    MiniView {
        visible: window.mini
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing * scale
        snapshot: window.mini ? window.snapshot : null
        groups: window.mini ? window.processes : null
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

        NavRail {
            pages: window.pages
            currentPage: window.currentPage
            collapsed: window.sidebarCollapsed
            onPageRequested: (index) => window.currentPage = index
            onMiniRequested: window.enterMini()
            onToggleRequested: {
                if (window.narrow) {
                    window.sidebarOpenedNarrow = !window.sidebarOpenedNarrow
                } else {
                    window.sidebarFolded = !window.sidebarFolded
                }
            }
        }

        // Content on the "view" background, lighter than the window chrome,
        // in a panel with a rounded corner as in Windows 11.
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.topMargin: Kirigami.Units.smallSpacing
            // The border shows on the top and left sides only.
            Layout.rightMargin: -1
            Layout.bottomMargin: -1
            Kirigami.Theme.colorSet: Kirigami.Theme.View
            Kirigami.Theme.inherit: false
            color: Kirigami.Theme.backgroundColor
            topLeftRadius: Kirigami.Units.cornerRadius * 2
            border.width: 1
            border.color: Qt.alpha(Kirigami.Theme.textColor, 0.08)

            StackLayout {
                id: stack
                anchors.fill: parent
                currentIndex: window.currentPage

                ProcessesPage {
                    onJournalRequested: (title, query) => journalDialog.show(title, query)
                    groups: window.shown(0, window.processes)
                    processDetails: window.shown(0, window.processDetails) ?? ({})
                    snapshot: window.shown(0, window.snapshot)
                    monitor: monitor
                    expandFirst: window.initialSelection.startsWith("expand:") ? window.initialSelection.slice(7) : ""
                }
                PerformancePage {
                    snapshot: window.shown(1, window.snapshot)
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
                    onJournalRequested: (title, query) => journalDialog.show(title, query)
                    groups: window.shown(2, window.processes)
                    processDetails: window.shown(2, window.processDetails) ?? ({})
                    monitor: monitor
                    logical: window.snapshot?.cpu.logical ?? 1
                }
                StartupPage {
                    monitor: monitor
                    active: window.currentPage === 3
                }
                UsersPage {
                    groups: window.shown(4, window.processes)
                    logical: window.snapshot?.cpu.logical ?? 1
                    onShowUser: (user) => {
                        details.query = user
                        window.currentPage = 2
                    }
                }
                ServicesPage {
                    monitor: monitor
                    onJournalRequested: (title, query) => journalDialog.show(title, query)
                    active: window.currentPage === 5
                }
                ConnectionsPage {
                    monitor: monitor
                    onJournalRequested: (title, query) => journalDialog.show(title, query)
                    groups: window.shown(6, window.processes)
                    active: window.currentPage === 6
                    onShowProcess: (pid) => {
                        details.query = String(pid)
                        window.currentPage = 2
                    }
                }
                CrashesPage {
                    monitor: monitor
                    active: window.currentPage === 7
                    openedAt: window.openedAt
                    bootedAt: window.openedAt - (window.snapshot?.cpu.uptime_secs ?? 0) * 1000
                    onJournalRequested: (title, query) => journalDialog.show(title, query)
                }
                BenchmarkPage {
                    monitor: monitor
                    active: window.currentPage === 8
                    snapshot: window.shown(8, window.snapshot)
                    // Development aid: TASKMGR_SELECT=run:all.
                    Component.onCompleted: if (window.initialSelection === "run:all") monitor.startBenchmark("all", folder)
                }
                SystemPage {
                    info: window.systemInfo
                    monitor: monitor
                    active: window.currentPage === 9
                    snapshot: window.shown(9, window.snapshot)
                }
                SettingsPage {
                    pageNames: window.pages.slice(0, window.pages.length - 1).map(p => p.text)
                }
            }
        }
    }
}
