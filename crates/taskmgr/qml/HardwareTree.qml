import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Hardware inventory as a tree (firmware, board, processor, memory modules,
// graphics, storage, network, displays, audio, USB, PCI), with the live
// readings of each component and their range since the window opened.
ColumnLayout {
    id: tree

    required property var monitor
    property var readings: []
    property bool active: false
    // Nodes whose open state differs from the default.
    property var toggled: ({})

    readonly property real unit: Kirigami.Units.gridUnit
    readonly property var parsed: monitor.inventory !== "" ? JSON.parse(monitor.inventory) : []
    // Readings no component claims (board chips, batteries, fans) get a
    // node of their own.
    readonly property var inventory: {
        const claimed = new Set()
        const walk = nodes => nodes.forEach(n => {
            if (n.live) {
                claimed.add(n.live)
            }
            walk(n.children ?? [])
        })
        walk(parsed)
        const rest = readings.filter(g => !claimed.has(g.key)).map(g => ({ label: g.name, live: g.key }))
        return parsed.length > 0 && rest.length > 0 ? parsed.concat([{ label: "Sensors", children: rest }]) : parsed
    }
    readonly property var power: monitor.power !== "" ? JSON.parse(monitor.power) : null
    readonly property string query: search.text.trim().toLowerCase()
    readonly property var profileNames: ({
        "power-saver": qsTr("Power saver"),
        "balanced": qsTr("Balanced"),
        "performance": qsTr("Performance"),
    })
    readonly property var liveByKey: {
        const map = {}
        for (const g of readings) {
            map[g.key] = g.readings.map(r => ({ label: r.label, reading: r }))
        }
        return map
    }
    readonly property var rows: {
        const all = []
        flatten(inventory, 0, "", [], all, query !== "")
        if (query === "") {
            return all
        }
        // Matches with their ancestors and everything under them.
        const keep = new Set()
        const under = []
        for (const r of all) {
            if (under.some(k => r.key.startsWith(k + "/"))) {
                keep.add(r.key)
                continue
            }
            if ((r.label + " " + Tr.core(r.label) + " " + valueText(r)).toLowerCase().includes(query)) {
                under.push(r.key)
                let k = r.key
                while (k) {
                    keep.add(k)
                    k = k.slice(0, k.lastIndexOf("/"))
                }
            }
        }
        return all.filter(r => keep.has(r.key))
    }

    spacing: 0

    function childrenOf(n) {
        const kids = n.children ?? []
        const live = n.live ? liveByKey[n.live] ?? [] : []
        if (live.length === 0 || kids.length === 0) {
            return live.length > 0 ? live : kids
        }
        // After the fields of the component, before its sub-components.
        const at = kids.findIndex(k => (k.children ?? []).length > 0)
        const node = { label: "Live readings", children: live, isLive: true }
        return at < 0 ? kids.concat([node]) : kids.slice(0, at).concat([node], kids.slice(at))
    }

    function isOpen(key, depth, n) {
        const byDefault = depth === 0 || !!n.isLive
        return byDefault !== !!toggled[key]
    }

    function flatten(nodes, depth, path, through, out, everything) {
        nodes.forEach((n, i) => {
            const key = path + "/" + i
            const kids = childrenOf(n)
            const last = i === nodes.length - 1
            const open = everything || isOpen(key, depth, n)
            out.push({ key: key, depth: depth, label: n.label, text: n.text, number: n.number, unit: n.unit,
                       reading: n.reading, hasChildren: kids.length > 0, open: open,
                       guide: { through: through, last: last } })
            if (open && kids.length > 0) {
                flatten(kids, depth + 1, key, through.concat([!last]), out, everything)
            }
        })
    }

    function format(unit, v) {
        switch (unit) {
        case "percent": return Format.percent(v, 1)
        case "mhz": return Format.mhz(v)
        case "mts": return qsTr("%1 MT/s").arg(Format.num(v, 0))
        case "celsius": return Format.temperature(v)
        case "watts": return Format.watts(v)
        case "volts": return Format.num(v, 3) + " V"
        case "rpm": return qsTr("%1 RPM").arg(Format.num(v, 0))
        case "bytes": return Format.bytes(v)
        case "bytes/s": return Format.rate(v)
        case "mbps": return Format.link(v)
        }
        return Format.num(v, 1)
    }

    function valueText(r) {
        if (r.reading) {
            return format(r.reading.unit, r.reading.value)
        }
        if (r.number !== undefined) {
            return format(r.unit, r.number)
        }
        return r.text !== undefined ? Tr.core(r.text) : ""
    }

    function asText() {
        const lines = []
        const walk = (nodes, depth) => {
            for (const n of nodes) {
                const v = n.reading ? format(n.reading.unit, n.reading.value)
                    : n.number !== undefined ? format(n.unit, n.number) : n.text !== undefined ? Tr.core(n.text) : ""
                lines.push("  ".repeat(depth) + Tr.core(n.label) + (v ? " : " + v : ""))
                walk(childrenOf(n), depth + 1)
            }
        }
        walk(inventory, 0)
        return lines.join("\n")
    }

    function focusSearch() {
        search.forceActiveFocus()
        search.selectAll()
    }

    onActiveChanged: if (active) monitor.refreshInventory()

    // The profile can change from the desktop's own controls.
    Timer {
        interval: 10000
        repeat: true
        triggeredOnStart: true
        running: tree.active
        onTriggered: tree.monitor.refreshPower()
    }

    RowLayout {
        spacing: Kirigami.Units.largeSpacing
        Layout.leftMargin: tree.unit
        Layout.rightMargin: tree.unit
        Layout.bottomMargin: Kirigami.Units.smallSpacing

        Kirigami.SearchField {
            id: search
            placeholderText: qsTr("Search the hardware")
            Layout.preferredWidth: tree.unit * 14
        }
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
        Item {
            Layout.fillWidth: true
        }
    }

    DataTable {
        id: table
        rows: tree.rows
        keyOf: r => r.key
        ordered: true
        indentUnit: chevron
        placeholder: tree.parsed.length === 0 ? qsTr("Reading the hardware…") : qsTr("Nothing matches")
        Layout.fillWidth: true
        Layout.fillHeight: true
        columns: [
            { key: "label", title: qsTr("Component"), maxWidth: tree.unit * 22, guides: true,
              indent: r => r.depth * table.indentUnit,
              expander: r => r.hasChildren ? r.open : null,
              text: r => Tr.core(r.label), dim: r => r.label === "Live readings" },
            { key: "value", title: qsTr("Value"), width: 0, text: r => tree.valueText(r) },
            { key: "min", title: qsTr("Min."), align: Text.AlignRight,
              text: r => r.reading ? tree.format(r.reading.unit, r.reading.min) : "" },
            { key: "max", title: qsTr("Max."), align: Text.AlignRight,
              text: r => r.reading ? tree.format(r.reading.unit, r.reading.max) : "" },
        ]
        onToggled: (row) => {
            if (tree.query !== "") {
                return
            }
            const t = Object.assign({}, tree.toggled)
            t[row.key] = !t[row.key]
            tree.toggled = t
        }
        onActivated: (row) => {
            if (row.hasChildren) {
                table.toggled(row)
            }
        }
    }
}
