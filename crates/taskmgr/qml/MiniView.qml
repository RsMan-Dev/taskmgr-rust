import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Compact readout: one line per resource and the busiest applications.
// `scale` enlarges everything for the full screen (F12) view.
ColumnLayout {
    id: mini

    property var snapshot: null
    property var groups: null
    property bool pinned: false
    property real scale: 1

    signal expandRequested()
    signal pinToggled()

    readonly property real unit: Kirigami.Units.gridUnit * scale
    readonly property var meters: {
        const s = snapshot
        if (!s) {
            return []
        }
        const busiestDisk = s.disks.reduce((a, d) => !a || d.active > a.active ? d : a, null)
        const rx = s.networks.reduce((a, n) => a + n.rx_bps + n.tx_bps, 0)
        const list = [
            { label: qsTr("CPU"), text: Format.percent(s.cpu.usage), values: s.cpu.history, max: 100, color: Kirigami.Theme.highlightColor },
            { label: qsTr("Memory"), text: Format.percent(s.memory.usage), values: s.memory.history, max: 100, color: "#9b59b6" },
        ]
        for (const g of s.gpus.filter(g => g.usage !== null)) {
            list.push({ label: qsTr("GPU %1").arg(g.index), text: Format.percent(g.usage), values: g.history, max: 100, color: "#1abc9c" })
        }
        if (busiestDisk) {
            list.push({ label: qsTr("Disk"), text: Format.percent(busiestDisk.active), values: busiestDisk.active_history, max: 100, color: "#1d99f3" })
        }
        if (s.networks.length > 0) {
            const hist = s.networks[0].rx_history
            list.push({ label: qsTr("Network"), text: Format.bits(rx), values: hist, max: Math.max(1, ...hist), color: "#f67400" })
        }
        if (s.cpu.temperature !== null) {
            list.push({ label: qsTr("Temperature"), text: Format.temperature(s.cpu.temperature),
                        values: s.cpu.charts.temperature ?? [], max: 100, color: "#da4453" })
        }
        return list
    }
    readonly property var busiest: (groups ?? []).slice().sort((a, b) => b.cpu - a.cpu).slice(0, 5)

    spacing: Kirigami.Units.smallSpacing * scale

    RowLayout {
        Layout.fillWidth: true
        Kirigami.Icon {
            source: Icons.logo
            Layout.preferredWidth: mini.unit * 1.2
            Layout.preferredHeight: mini.unit * 1.2
        }
        QQC2.Label {
            text: Format.uptime(mini.snapshot?.cpu.uptime_secs ?? 0)
            opacity: 0.6
            font.pointSize: Kirigami.Theme.smallFont.pointSize * mini.scale
            Layout.fillWidth: true
        }
        QQC2.ToolButton {
            visible: mini.scale === 1
            icon.source: Icons.of("pin", Kirigami.Theme.textColor)
            checkable: true
            checked: mini.pinned
            display: QQC2.AbstractButton.IconOnly
            text: qsTr("Always on top")
            onClicked: mini.pinToggled()
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
        }
        QQC2.ToolButton {
            icon.source: Icons.of("maximize", Kirigami.Theme.textColor)
            display: QQC2.AbstractButton.IconOnly
            text: qsTr("Full view")
            onClicked: mini.expandRequested()
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
        }
    }

    Repeater {
        model: mini.meters
        RowLayout {
            required property var modelData
            spacing: Kirigami.Units.largeSpacing * mini.scale
            Layout.fillWidth: true
            QQC2.Label {
                text: modelData.label
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * mini.scale
                elide: Text.ElideRight
                Layout.preferredWidth: mini.unit * 5
            }
            UsageChart {
                compact: true
                series: [{ values: modelData.values, color: modelData.color }]
                maximum: modelData.max
                accent: modelData.color
                Layout.fillWidth: true
                Layout.preferredHeight: mini.unit * 1.4
            }
            QQC2.Label {
                text: modelData.text
                horizontalAlignment: Text.AlignRight
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * mini.scale
                font.weight: Font.DemiBold
                font.features: { "tnum": 1 }
                Layout.preferredWidth: mini.unit * 4.5
            }
        }
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    Repeater {
        model: mini.busiest
        RowLayout {
            required property var modelData
            Layout.fillWidth: true
            Kirigami.Icon {
                readonly property bool own: !modelData.icon
                source: own ? Icons.of(modelData.kind, Kirigami.Theme.textColor) : modelData.icon
                Layout.preferredWidth: mini.unit
                Layout.preferredHeight: mini.unit
            }
            QQC2.Label {
                text: Tr.core(modelData.name)
                elide: Text.ElideRight
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * mini.scale
                Layout.fillWidth: true
            }
            QQC2.Label {
                text: Format.percent(modelData.cpu, 1)
                opacity: 0.7
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * mini.scale
                font.features: { "tnum": 1 }
            }
        }
    }

    Item {
        Layout.fillHeight: true
    }
}
