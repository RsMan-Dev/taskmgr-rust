import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Every live reading grouped by component, with its lowest and highest
// value since the window opened, plus the CPU power settings.
ColumnLayout {
    id: tree

    required property var monitor
    property var readings: []
    property bool active: false

    readonly property real unit: Kirigami.Units.gridUnit
    readonly property var power: monitor.power !== "" ? JSON.parse(monitor.power) : null
    readonly property var profileNames: ({
        "power-saver": qsTr("Power saver"),
        "balanced": qsTr("Balanced"),
        "performance": qsTr("Performance"),
    })
    readonly property var rows: {
        const out = []
        for (const g of readings) {
            for (const r of g.readings) {
                out.push({ key: r.key, group: g.key, groupName: g.kind === "memory" ? qsTr("Memory") : Tr.core(g.name),
                           label: r.label, unit: r.unit, value: r.value, min: r.min, max: r.max })
            }
        }
        return out
    }

    spacing: 0

    function format(unit, v) {
        switch (unit) {
        case "percent": return Format.percent(v, 1)
        case "mhz": return Format.mhz(v)
        case "celsius": return Format.temperature(v)
        case "watts": return Format.watts(v)
        case "volts": return Format.num(v, 3) + " V"
        case "rpm": return qsTr("%1 RPM").arg(Format.num(v, 0))
        case "bytes": return Format.bytes(v)
        case "bytes/s": return Format.rate(v)
        }
        return Format.num(v, 1)
    }

    function label(text) {
        return Tr.core(text)
    }

    function asText() {
        let group = ""
        const lines = []
        for (const r of rows) {
            if (r.group !== group) {
                group = r.group
                lines.push(r.groupName)
            }
            lines.push("  " + label(r.label) + " : " + format(r.unit, r.value)
                + "  (" + format(r.unit, r.min) + " - " + format(r.unit, r.max) + ")")
        }
        return lines.join("\n")
    }

    // The profile can change from the desktop's own controls.
    Timer {
        interval: 10000
        repeat: true
        triggeredOnStart: true
        running: tree.active
        onTriggered: tree.monitor.refreshPower()
    }

    RowLayout {
        visible: tree.power !== null
        spacing: Kirigami.Units.largeSpacing
        Layout.leftMargin: tree.unit
        Layout.rightMargin: tree.unit
        Layout.bottomMargin: Kirigami.Units.smallSpacing

        QQC2.Label {
            visible: !!tree.power?.profile
            text: qsTr("Power profile")
        }
        QQC2.ComboBox {
            readonly property var profiles: tree.power?.profiles ?? []
            visible: !!tree.power?.profile
            model: profiles.map(p => tree.profileNames[p] ?? p)
            implicitContentWidthPolicy: QQC2.ComboBox.WidestText
            currentIndex: profiles.indexOf(tree.power?.profile ?? "")
            onActivated: (i) => tree.monitor.setPowerProfile(profiles[i])
        }
        QQC2.Label {
            text: [tree.power?.driver ? qsTr("Driver: %1").arg(tree.power.driver) : "",
                   tree.power?.governor ? qsTr("Governor: %1").arg(tree.power.governor) : "",
                   tree.power?.epp ? qsTr("Energy preference: %1").arg(tree.power.epp) : "",
                   tree.power?.degraded ? qsTr("Performance held back: %1").arg(tree.power.degraded) : ""]
                .filter(x => x).join("  ·  ")
            opacity: 0.7
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
    }

    DataTable {
        id: table
        rows: tree.rows
        keyOf: r => r.key
        groupBy: r => r.group
        groupRow: (key, list) => ({ name: list[0].groupName, count: list.length })
        openByDefault: true
        ordered: true
        placeholder: tree.readings.length === 0 ? qsTr("Reading the sensors…") : ""
        Layout.fillWidth: true
        Layout.fillHeight: true
        columns: [
            { key: "label", title: qsTr("Sensor"), width: 0,
              text: r => r._group !== undefined ? r.name : tree.label(r.label) },
            { key: "value", title: qsTr("Value"), align: Text.AlignRight,
              text: r => r._group !== undefined ? "" : tree.format(r.unit, r.value) },
            { key: "min", title: qsTr("Min."), align: Text.AlignRight,
              text: r => r._group !== undefined ? "" : tree.format(r.unit, r.min) },
            { key: "max", title: qsTr("Max."), align: Text.AlignRight,
              text: r => r._group !== undefined ? "" : tree.format(r.unit, r.max) },
        ]
    }
}
