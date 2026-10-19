import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ThinScrollView {
    id: page

    required property var pageNames

    readonly property var languages: [["", qsTr("System language")], ["en", "English"], ["fr", "Français"]]
    readonly property var intervals: [[500, qsTr("0.5 s")], [1000, qsTr("1 s")], [2000, qsTr("2 s")], [5000, qsTr("5 s")], [0, qsTr("Paused")]]
    property string startLanguage: ""
    property string startTheme: ""
    readonly property var themes: [["", qsTr("System")], ["light", qsTr("Light")], ["dark", qsTr("Dark")]]

    contentWidth: availableWidth
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
