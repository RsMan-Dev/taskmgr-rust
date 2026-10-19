import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ThinScrollView {
    id: page

    required property var pageNames

    readonly property var languages: [["", qsTr("System language")], ["en", "English"], ["fr", "Français"]]
    readonly property var intervals: [[250, qsTr("0.25 s")], [500, qsTr("0.5 s")], [1000, qsTr("1 s")], [2000, qsTr("2 s")],
                                      [5000, qsTr("5 s")], [10000, qsTr("10 s")], [0, qsTr("Paused")]]
    readonly property var points: [30, 60, 120, 300]
    property string startLanguage: ""
    property string startTheme: ""
    readonly property var themes: [["", qsTr("System")], ["light", qsTr("Light")], ["dark", qsTr("Dark")]]

    contentWidth: availableWidth
    // Check box labels wrap instead of running off a narrow window.
    readonly property real textWidth: Math.max(Kirigami.Units.gridUnit * 10, availableWidth - Kirigami.Units.gridUnit * (Kirigami.Settings.isMobile || availableWidth < Kirigami.Units.gridUnit * 30 ? 4 : 18))
    Component.onCompleted: {
        startLanguage = Prefs.language
        startTheme = Prefs.theme
    }

    ColumnLayout {
        width: page.availableWidth
        spacing: Kirigami.Units.largeSpacing

        PageHeader {
            title: qsTr("Settings")
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.gridUnit

            QQC2.ComboBox {
                Kirigami.FormData.label: qsTr("Language:")
                model: page.languages.map(l => l[1])
                currentIndex: Math.max(0, page.languages.findIndex(l => l[0] === Prefs.language))
                onActivated: (i) => Prefs.language = page.languages[i][0]
            }
            QQC2.Label {
                visible: Prefs.language !== page.startLanguage
                text: qsTr("Takes effect at the next start.")
                opacity: 0.7
                font: Kirigami.Theme.smallFont
            }

            QQC2.ComboBox {
                Kirigami.FormData.label: qsTr("Theme:")
                model: page.themes.map(t => t[1])
                currentIndex: Math.max(0, page.themes.findIndex(t => t[0] === Prefs.theme))
                onActivated: (i) => Prefs.theme = page.themes[i][0]
            }
            QQC2.Label {
                visible: Prefs.theme !== page.startTheme
                text: qsTr("Takes effect at the next start.")
                opacity: 0.7
                font: Kirigami.Theme.smallFont
            }

            QQC2.ComboBox {
                Kirigami.FormData.label: qsTr("Update speed:")
                model: page.intervals.map(i => i[1])
                currentIndex: Math.max(0, page.intervals.findIndex(i => i[0] === Prefs.intervalMs))
                onActivated: (i) => Prefs.intervalMs = page.intervals[i][0]
            }

            QQC2.ComboBox {
                Kirigami.FormData.label: qsTr("Start page:")
                model: page.pageNames
                currentIndex: Prefs.startPage
                onActivated: (i) => Prefs.startPage = i
            }

            QQC2.CheckBox {
                Layout.maximumWidth: page.textWidth
                Kirigami.FormData.label: qsTr("CPU:")
                text: qsTr("Show process CPU as % of one core (like top)")
                checked: Prefs.cpuPerCore
                onToggled: Prefs.cpuPerCore = checked
            }

            QQC2.Label {
                Kirigami.FormData.label: qsTr("Columns:")
                text: qsTr("Right click a column header of the process list.")
                opacity: 0.7
                wrapMode: Text.Wrap
                Layout.maximumWidth: Kirigami.Units.gridUnit * 22
            }
            QQC2.CheckBox {
                Layout.maximumWidth: page.textWidth
                text: qsTr("Grey out values that are zero")
                checked: Prefs.dimZeros
                onToggled: Prefs.dimZeros = checked
            }
            QQC2.CheckBox {
                Layout.maximumWidth: page.textWidth
                text: qsTr("Show column separators")
                checked: Prefs.columnSeparators
                onToggled: Prefs.columnSeparators = checked
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Graphs")
            }
            QQC2.ComboBox {
                Kirigami.FormData.label: qsTr("Points per graph:")
                model: page.points.map(p => qsTr("%1 (%2)").arg(p).arg(Format.span(p * Math.max(Prefs.intervalMs, 250) / 1000)))
                currentIndex: Math.max(0, page.points.indexOf(Prefs.historyPoints))
                onActivated: (i) => Prefs.historyPoints = page.points[i]
            }
            QQC2.CheckBox {
                Layout.maximumWidth: page.textWidth
                text: qsTr("Smooth curves")
                checked: Prefs.smoothGraphs
                onToggled: Prefs.smoothGraphs = checked
            }
            QQC2.CheckBox {
                Layout.maximumWidth: page.textWidth
                text: qsTr("Show kernel times in the CPU graphs")
                checked: Prefs.kernelTimes
                onToggled: Prefs.kernelTimes = checked
            }
            QQC2.CheckBox {
                Layout.maximumWidth: page.textWidth
                text: qsTr("Scale network graphs to the link speed")
                checked: Prefs.networkScaleLink
                onToggled: Prefs.networkScaleLink = checked
            }
            QQC2.CheckBox {
                Layout.maximumWidth: page.textWidth
                Kirigami.FormData.label: qsTr("Network:")
                text: qsTr("Also show VPN, bridges and virtual adapters")
                checked: Prefs.showVirtualNetworks
                onToggled: Prefs.showVirtualNetworks = checked
            }
            QQC2.Label {
                Kirigami.FormData.label: qsTr("Performance list:")
                text: qsTr("Right click an entry to hide it or move it.")
                opacity: 0.7
                wrapMode: Text.Wrap
                Layout.maximumWidth: Kirigami.Units.gridUnit * 22
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("Units")
            }
            QQC2.ComboBox {
                Kirigami.FormData.label: qsTr("Sizes:")
                model: [qsTr("Powers of 1024 (1 KB = 1024 B)"), qsTr("Powers of 1000 (1 kB = 1000 B)")]
                currentIndex: Prefs.sizeBase10 ? 1 : 0
                onActivated: (i) => Prefs.sizeBase10 = i === 1
            }
            QQC2.ComboBox {
                Kirigami.FormData.label: qsTr("Network speed:")
                model: [qsTr("Bits per second (Mb/s)"), qsTr("Bits, powers of 1024 (Mib/s)"), qsTr("Bytes per second (MB/s)")]
                currentIndex: Prefs.networkBytes ? 2 : Prefs.networkBase2 ? 1 : 0
                onActivated: (i) => {
                    Prefs.networkBytes = i === 2
                    Prefs.networkBase2 = i === 1
                }
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: qsTr("About")
            }
            Kirigami.Icon {
                source: Icons.logo
                Layout.preferredWidth: Kirigami.Units.iconSizes.large
                Layout.preferredHeight: Kirigami.Units.iconSizes.large
            }
            QQC2.Label {
                Kirigami.FormData.label: qsTr("Application:")
                text: "taskmgr-rust 0.1.0  (fr.rsman.taskmgr-rust)"
            }
            QQC2.Label {
                Kirigami.FormData.label: qsTr("License:")
                text: "MIT"
            }
        }
    }
}
