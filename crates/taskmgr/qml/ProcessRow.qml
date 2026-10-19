import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

QQC2.ItemDelegate {
    id: row

    required property var entry
    property bool selected: false
    property bool expanded: false
    property real columnWidth: Kirigami.Units.gridUnit * 6

    signal toggleRequested()
    signal menuRequested(point position)

    readonly property bool isGroup: entry.type === "group"
    readonly property var group: entry.group
    readonly property var proc: entry.proc
    readonly property bool expandable: isGroup && group.processes.length > 1
    readonly property var metrics: isGroup ? group : proc
    readonly property bool secondary: !isGroup && proc.role !== "normal"

    padding: 0
    hoverEnabled: true

    background: Rectangle {
        color: row.selected ? Qt.alpha(Kirigami.Theme.highlightColor, 0.22)
             : row.hovered ? Qt.alpha(Kirigami.Theme.highlightColor, 0.08) : "transparent"
    }

    onDoubleClicked: if (expandable) toggleRequested()

    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: (point) => row.menuRequested(point.position)
    }

    contentItem: RowLayout {
        spacing: 0

        RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: row.isGroup ? Kirigami.Units.smallSpacing : Kirigami.Units.gridUnit * 2.6
            Layout.rightMargin: Kirigami.Units.smallSpacing

            QQC2.ToolButton {
                visible: row.isGroup
                enabled: row.expandable
                opacity: row.expandable ? 1 : 0
                icon.name: row.expanded ? "arrow-down" : "arrow-right"
                icon.width: Kirigami.Units.iconSizes.small
                icon.height: Kirigami.Units.iconSizes.small
                display: QQC2.AbstractButton.IconOnly
                flat: true
                text: row.expanded ? "Réduire" : "Développer"
                onClicked: row.toggleRequested()
                Layout.preferredWidth: Kirigami.Units.gridUnit * 1.4
                Layout.preferredHeight: Kirigami.Units.gridUnit * 1.4
            }

            Kirigami.Icon {
                visible: row.isGroup
                readonly property string generic: row.isGroup && row.group.kind === "app" ? "application-x-executable" : "system-run"
                source: row.isGroup && row.group.icon ? row.group.icon : generic
                fallback: generic
                Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
            }

            QQC2.Label {
                text: row.isGroup
                    ? row.group.name + (row.expandable ? "  (" + row.group.processes.length + ")" : "")
                    : row.proc.name
                elide: Text.ElideRight
                opacity: row.secondary ? 0.6 : 1
                Layout.fillWidth: implicitWidth > width
                Layout.maximumWidth: implicitWidth
                Layout.minimumWidth: Kirigami.Units.gridUnit * 4
            }

            Repeater {
                model: row.isGroup ? row.group.via : []
                Chip {
                    required property string modelData
                    text: modelData
                }
            }

            Chip {
                visible: row.secondary
                text: row.secondary && row.proc.role === "wrapper" ? "enveloppe" : "Wine"
            }

            QQC2.Label {
                visible: !row.isGroup || !row.expandable
                text: "PID " + (row.isGroup ? row.group.processes[0].pid : row.proc.pid)
                opacity: 0.5
                font: Kirigami.Theme.smallFont
            }

            Item { Layout.fillWidth: true }
        }

        HeatCell {
            value: row.metrics.cpu
            reference: 25
            text: Format.num(row.metrics.cpu, 1) + " %"
            emphasized: row.isGroup
            Layout.preferredWidth: row.columnWidth
            Layout.fillHeight: true
        }
        HeatCell {
            value: row.metrics.memory
            reference: 2 * 1024 * 1024 * 1024
            text: Format.bytes(row.metrics.memory)
            emphasized: row.isGroup
            Layout.preferredWidth: row.columnWidth
            Layout.fillHeight: true
        }
        HeatCell {
            value: row.metrics.disk_bps ?? 0
            reference: 20 * 1024 * 1024
            text: row.metrics.disk_bps === null ? "" : Format.num(row.metrics.disk_bps / 1048576, 1) + " Mo/s"
            emphasized: row.isGroup
            Layout.preferredWidth: row.columnWidth
            Layout.fillHeight: true
        }
    }

    component Chip: Rectangle {
        property alias text: chipLabel.text
        radius: height / 2
        color: Qt.alpha(Kirigami.Theme.textColor, 0.08)
        implicitWidth: chipLabel.implicitWidth + Kirigami.Units.largeSpacing * 2
        implicitHeight: chipLabel.implicitHeight + 2
        QQC2.Label {
            id: chipLabel
            anchors.centerIn: parent
            font: Kirigami.Theme.smallFont
            opacity: 0.8
        }
    }
}
