import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// TCP and UDP sockets, folded under the application that owns them.
ColumnLayout {
    id: page

    required property var monitor
    property var groups: null
    property bool active: false
    // 0: all, 1: connected, 2: listening.
    property int filter: 0
    property string error: ""

    signal showProcess(int pid)

    readonly property real unit: Kirigami.Units.gridUnit
    // pid -> { group, proc } from the process list.
    readonly property var owners: {
        const map = {}
        for (const g of groups ?? []) {
            for (const p of g.processes) {
                map[p.pid] = { group: g, proc: p }
            }
        }
        return map
    }
    readonly property var connections: {
        const all = monitor.connections !== "" ? JSON.parse(monitor.connections) : []
        // Several sockets can share addresses (multicast): number them.
        const seen = {}
        for (const c of all) {
            const k = [c.protocol, c.pid, c.local, c.local_port, c.remote, c.remote_port].join(" ")
            seen[k] = (seen[k] ?? 0) + 1
            c.id = k + " #" + seen[k]
        }
        const q = search.text.trim().toLowerCase()
        return all.filter(c => {
            if (filter === 1 && c.state !== "established") {
                return false
            }
            if (filter === 2 && !(c.state === "listen" || (c.protocol === "UDP" && c.remote === ""))) {
                return false
            }
            return q === "" || [appName(c), c.pid ?? "", endpoint(c.local, c.local_port), endpoint(c.remote, c.remote_port)]
                .some(x => String(x).toLowerCase().includes(q))
        })
    }

    spacing: 0

    function appName(c) {
        if (c._group !== undefined) {
            return c.name
        }
        const o = c.pid !== null ? owners[c.pid] : null
        if (c._child && o) {
            return o.proc.name
        }
        return c.pid === null ? qsTr("No application (%1)").arg(c.user) : Tr.core(o?.group.name ?? "") || qsTr("PID %1").arg(c.pid)
    }

    // Sockets fold under their application; unknown ones under their user.
    function groupOf(c) {
        const o = c.pid !== null ? owners[c.pid] : null
        return o ? "app:" + o.group.key : c.pid !== null ? "pid:" + c.pid : "user:" + c.user
    }

    function groupRow(key, list) {
        const first = list[0]
        return {
            name: appName(first),
            count: list.length,
            known: first.pid !== null,
            group: first.pid !== null ? owners[first.pid]?.group ?? null : null,
            pid: null, protocol: "", ipv6: false, local: "", local_port: 0, remote: "", remote_port: 0,
            state: "", user: first.user,
        }
    }

    // "pid:start" pairs, so a reused pid is left alone; the whole
    // application for a group row, as on the Processes page.
    function targetsOf(row) {
        if (row._group !== undefined) {
            return row.group ? row.group.processes.map(p => p.pid + ":" + p.start).join(",") : ""
        }
        const p = row.pid !== null ? owners[row.pid]?.proc : null
        return p ? p.pid + ":" + p.start : ""
    }

    function endTask(row, force) {
        const targets = row ? targetsOf(row) : ""
        if (targets !== "") {
            error = monitor.sendSignal(targets, force ? "kill" : "term") === "denied"
                ? qsTr("Access denied: this process belongs to another user or to the system.") : ""
        }
    }

    function endpoint(address, port) {
        if (address === "") {
            return ""
        }
        return (address.includes(":") ? "[" + address + "]" : address) + ":" + port
    }

    function stateName(c) {
        if (c._group !== undefined) {
            return ""
        }
        switch (c.state) {
        case "established": return qsTr("Connected")
        case "listen": return qsTr("Listening")
        case "syn-sent": case "syn-recv": return qsTr("Connecting")
        case "fin-wait": case "time-wait": case "close-wait": case "last-ack": case "closing": case "close":
            return qsTr("Closing")
        }
        return c.protocol === "UDP" ? qsTr("Open") : c.state
    }

    onActiveChanged: if (active) monitor.refreshConnections()

    Timer {
        interval: 2000
        repeat: true
        running: page.active
        onTriggered: page.monitor.refreshConnections()
    }

    PageHeader {
        title: qsTr("Connections")

        QQC2.ComboBox {
            model: [qsTr("All"), qsTr("Connected"), qsTr("Listening")]
            currentIndex: page.filter
            onActivated: (i) => page.filter = i
        }
        Kirigami.SearchField {
            id: search
            placeholderText: qsTr("Search an app, address or port")
            Layout.fillWidth: true
            Layout.preferredWidth: page.unit * 14
            Layout.maximumWidth: page.unit * 14
            Layout.minimumWidth: page.unit * 6
        }
    }

    QQC2.Label {
        text: qsTr("Sockets of other users are listed without their application.")
        opacity: 0.6
        font: Kirigami.Theme.smallFont
        wrapMode: Text.Wrap
        Layout.fillWidth: true
        Layout.leftMargin: page.unit
        Layout.rightMargin: page.unit
        Layout.bottomMargin: Kirigami.Units.smallSpacing
    }

    Kirigami.InlineMessage {
        visible: page.error !== ""
        text: page.error
        type: Kirigami.MessageType.Error
        showCloseButton: true
        onVisibleChanged: if (!visible) page.error = ""
        Layout.fillWidth: true
        Layout.leftMargin: page.unit
        Layout.rightMargin: page.unit
        Layout.bottomMargin: Kirigami.Units.smallSpacing
    }

    DataTable {
        id: table
        rows: page.connections
        keyOf: c => c.id
        groupBy: c => page.groupOf(c)
        groupRow: (key, list) => page.groupRow(key, list)
        sortColumn: "app"
        placeholder: page.monitor.connections === "" ? "" : qsTr("No matching connection")
        Layout.fillWidth: true
        Layout.fillHeight: true
        columns: [
            { key: "app", title: qsTr("Application"), maxWidth: page.unit * 20,
              text: c => c._group !== undefined ? qsTr("%1 (%2)").arg(c.name).arg(c.count) : page.appName(c),
              value: c => ((c._group !== undefined ? c.known : c.pid !== null) ? "" : "\uffff") + page.appName(c), dim: c => c._group !== undefined ? !c.known : c.pid === null },
            { key: "pid", title: qsTr("PID"), align: Text.AlignRight, text: c => c.pid ?? "", value: c => c.pid ?? -1 },
            { key: "protocol", title: qsTr("Protocol"), text: c => c.protocol + (c.ipv6 ? "v6" : "") },
            { key: "local", title: qsTr("Local address"), text: c => page.endpoint(c.local, c.local_port) },
            { key: "remote", title: qsTr("Remote address"), text: c => page.endpoint(c.remote, c.remote_port) },
            { key: "state", title: qsTr("State"), text: c => page.stateName(c) },
            { key: "user", title: qsTr("User"), width: 0, text: c => c.user },
        ]
        onActivated: (c) => {
            if (c._group !== undefined) {
                table.toggle(c._group)
            } else if (c.pid !== null) {
                page.showProcess(c.pid)
            }
        }
        onContextMenuRequested: (row, item, position) => {
            menu.row = row
            menu.popup(item, position)
        }
    }

    QQC2.Menu {
        id: menu
        property var row: null
        readonly property bool actionable: row !== null && page.targetsOf(row) !== ""

        QQC2.MenuItem {
            text: qsTr("End task")
            icon.source: Icons.of("end-task", Kirigami.Theme.textColor)
            enabled: menu.actionable
            onTriggered: page.endTask(menu.row, false)
        }
        QQC2.MenuItem {
            text: qsTr("Kill task")
            icon.source: Icons.of("kill", Kirigami.Theme.textColor)
            enabled: menu.actionable
            onTriggered: page.endTask(menu.row, true)
        }
        QQC2.MenuSeparator {}
        QQC2.MenuItem {
            visible: menu.row?._group !== undefined
            height: visible ? implicitHeight : 0
            text: table.expanded[menu.row?._group] ? qsTr("Collapse") : qsTr("Expand")
            icon.source: Icons.of("collapse", Kirigami.Theme.textColor)
            onTriggered: table.toggle(menu.row._group)
        }
        QQC2.MenuItem {
            text: qsTr("Go to details")
            icon.source: Icons.of("nav-details", Kirigami.Theme.textColor)
            enabled: menu.row !== null && (menu.row._group !== undefined ? !!menu.row.group : menu.row.pid !== null)
            onTriggered: page.showProcess(menu.row._group !== undefined ? menu.row.group.processes[0].pid : menu.row.pid)
        }
    }
}
