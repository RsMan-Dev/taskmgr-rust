import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// System: a summary of the machine, and every live reading with its range.
ColumnLayout {
    id: page

    required property var info
    required property var monitor
    property var snapshot: null
    property bool active: false

    spacing: 0

    PageHeader {
        title: qsTr("System")

        QQC2.TabBar {
            id: tabs
            QQC2.TabButton {
                text: qsTr("Summary")
                width: implicitWidth
            }
            QQC2.TabButton {
                text: qsTr("Details")
                width: implicitWidth
            }
        }
        QQC2.Button {
            text: copied.running ? qsTr("Copied") : qsTr("Copy")
            icon.source: Icons.of(copied.running ? "copied" : "copy", Kirigami.Theme.textColor)
            onClicked: {
                clipboard.text = tabs.currentIndex === 0 ? summary.asText() : tree.asText()
                clipboard.selectAll()
                clipboard.copy()
                copied.restart()
            }
            Timer {
                id: copied
                interval: 2000
            }
        }
    }

    TextEdit {
        id: clipboard
        visible: false
    }

    StackLayout {
        currentIndex: tabs.currentIndex
        Layout.fillWidth: true
        Layout.fillHeight: true

        SystemSummary {
            id: summary
            info: page.info
            snapshot: page.snapshot
        }
        SensorTree {
            id: tree
            monitor: page.monitor
            readings: tabs.currentIndex === 1 ? page.snapshot?.readings ?? [] : []
            active: page.active && tabs.currentIndex === 1
        }
    }
}
