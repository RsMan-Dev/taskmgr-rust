import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Every process in a flat list, Windows "Details" style.
ColumnLayout {
    id: page

    required property var groups
    required property var monitor
    property int logical: 1

    property alias query: search.text
    property string message: ""
    property var pendingPriority: null

    readonly property real unit: Kirigami.Units.gridUnit
    readonly property var priorities: [
        [-10, qsTr("High")],
        [-5, qsTr("Above normal")],
        [0, qsTr("Normal")],
        [5, qsTr("Below normal")],
        [10, qsTr("Low")],
        [19, qsTr("Idle")],
    ]

    property bool tree: false

    // Depth-first order under each parent, children sorted by pid.
    function treeOrder(list) {
        const byPid = {}
        const children = {}
        for (const p of list) {
            byPid[p.pid] = p
        }
        const roots = []
        for (const p of list) {
            if (byPid[p.ppid]) {
                if (!children[p.ppid]) {
                    children[p.ppid] = []
                }
                children[p.ppid].push(p)
            } else {
                roots.push(p)
            }
        }
        const out = []
        const walk = (p, depth) => {
            out.push(Object.assign({ depth: depth }, p))
            for (const c of (children[p.pid] ?? []).sort((a, b) => a.pid - b.pid)) {
                walk(c, depth + 1)
            }
        }
        for (const r of roots.sort((a, b) => a.pid - b.pid)) {
            walk(r, 0)
        }
        return out
    }

    readonly property var shown: tree ? treeOrder(processes) : processes

    readonly property var processes: {
        const out = []
        const q = search.text.trim().toLowerCase()
        for (const g of groups ?? []) {
            for (const p of g.processes) {
                if (q === "" || p.name.toLowerCase().includes(q) || String(p.pid) === q
                        || p.user.toLowerCase().includes(q) || p.command.toLowerCase().includes(q)) {
                    out.push(Object.assign({ app: Tr.core(g.name) }, p))
                }
            }
        }
        return out
    }

    spacing: 0

    function stateName(s) {
        switch (s) {
        case "R": return qsTr("Running")
        case "S": return qsTr("Sleeping")
        case "D": return qsTr("Waiting for I/O")
        case "T": return qsTr("Suspended")
        case "t": return qsTr("Traced")
        case "Z": return qsTr("Zombie")
        case "I": return qsTr("Idle")
        }
        return s
    }

    function priorityName(nice) {
        let best = priorities[0]
        for (const p of priorities) {
            if (Math.abs(p[0] - nice) < Math.abs(best[0] - nice)) {
                best = p
            }
        }
        return best[1] + " (" + nice + ")"
    }

    function signal(row, sig) {
        if (row && monitor.sendSignal(row.pid + ":" + row.start, sig) === "denied") {
            message = qsTr("Access denied: this process belongs to another user or to the system.")
        }
    }

    function setPriority(row, nice) {
        const r = monitor.setPriority(row.pid + ":" + row.start, nice)
        if (r === "denied") {
            pendingPriority = { pid: row.pid, nice: nice }
            message = qsTr("Raising the priority of %1 needs administrator rights.").arg(row.name)
        } else if (r !== "" && r !== "gone") {
            message = r
        }
    }

    function folder(path) {
        return path.substring(0, path.lastIndexOf("/")) || "/"
    }

    Connections {
        target: page.monitor
        function onActionResultChanged() {
            const r = JSON.parse(page.monitor.actionResult)
            if (r.action === "priority") {
                page.pendingPriority = null
                page.message = r.ok ? "" : Tr.core(r.message)
            }
        }
    }

    PageHeader {
        title: qsTr("Details")

        Kirigami.SearchField {
            id: search
            placeholderText: qsTr("Search a name, PID, user or command")
            Layout.fillWidth: true
            Layout.preferredWidth: page.unit * 16
            Layout.maximumWidth: page.unit * 16
            Layout.minimumWidth: page.unit * 6
        }
        QQC2.ToolButton {
            text: qsTr("Tree")
            icon.source: Icons.of("tree")
            icon.color: Kirigami.Theme.textColor
            display: QQC2.AbstractButton.IconOnly
            checkable: true
            checked: page.tree
            onToggled: page.tree = checked
            QQC2.ToolTip.text: qsTr("Show processes as a tree (parent, children)")
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }
        QQC2.Button {
            display: page.width < page.unit * 36 ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
            text: qsTr("End task")
            icon.source: Icons.of("end-task")
            icon.color: Kirigami.Theme.textColor
            enabled: table.selectedRow !== null
            onClicked: page.signal(table.selectedRow, "term")
        }
        QQC2.Button {
            display: page.width < page.unit * 36 ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
            text: qsTr("Kill")
            icon.source: Icons.of("kill")
            icon.color: Kirigami.Theme.textColor
            enabled: table.selectedRow !== null
            onClicked: page.signal(table.selectedRow, "kill")
        }
    }

    Kirigami.InlineMessage {
        visible: page.message !== ""
        text: page.message
        type: page.pendingPriority ? Kirigami.MessageType.Warning : Kirigami.MessageType.Error
        showCloseButton: true
        onVisibleChanged: if (!visible) {
            page.message = ""
            page.pendingPriority = null
        }
        actions: [
            Kirigami.Action {
                visible: page.pendingPriority !== null
                text: qsTr("Retry as administrator…")
                icon.source: Icons.of("admin")
                icon.color: Kirigami.Theme.textColor
                onTriggered: {
                    page.monitor.setPriorityAsAdmin(page.pendingPriority.pid, page.pendingPriority.nice)
                    page.message = qsTr("Waiting for the administrator password…")
                }
            }
        ]
        Layout.fillWidth: true
        Layout.leftMargin: page.unit
        Layout.rightMargin: page.unit
        Layout.bottomMargin: Kirigami.Units.smallSpacing
    }

    DataTable {
        id: table
        rows: page.shown
        ordered: page.tree
        keyOf: row => String(row.pid)
        sortColumn: "cpu"
        sortDescending: true
        placeholder: page.groups === null ? "" : qsTr("No matching process")
        Layout.fillWidth: true
        Layout.fillHeight: true
        columns: [
            { key: "name", title: qsTr("Name"), maxWidth: page.unit * 20, text: r => r.name, dim: r => r.state === "T",
              indent: r => page.tree ? (r.depth ?? 0) * page.unit : 0 },
            { key: "pid", title: qsTr("PID"), text: r => r.pid, value: r => r.pid, align: Qt.AlignRight, widest: ["4194304"] },
            { key: "state", title: qsTr("Status"), text: r => page.stateName(r.state) },
            { key: "user", title: qsTr("User"), text: r => r.user },
            { key: "cpu", title: qsTr("CPU"), value: r => r.cpu, widest: [Format.percent(100 * (Prefs.cpuPerCore ? page.logical : 1), 1)],
              text: r => Format.percent(r.cpu * (Prefs.cpuPerCore ? page.logical : 1), 1), align: Qt.AlignRight },
            { key: "memory", title: qsTr("Memory"), widest: [Format.bytes(999 * 1048576), Format.bytes(99.9 * 1073741824)], value: r => r.memory, text: r => Format.bytes(r.memory), align: Qt.AlignRight },
            { key: "threads", title: qsTr("Threads"), widest: ["99999"], value: r => r.threads, text: r => r.threads, align: Qt.AlignRight },
            { key: "nice", title: qsTr("Priority"), value: r => r.nice, text: r => page.priorityName(r.nice) },
            { key: "app", title: qsTr("Application"), maxWidth: page.unit * 12, text: r => r.app },
            { key: "command", title: qsTr("Command line"), width: 0, text: r => r.command || "[" + r.name + "]" },
        ]
        onContextMenuRequested: (row, item, position) => {
            menu.row = row
            menu.popup(item, position)
        }
    }

    QQC2.Menu {
        id: menu
        property var row: null

        QQC2.MenuItem {
            text: qsTr("End task")
            icon.source: Icons.of("end-task")
            icon.color: Kirigami.Theme.textColor
            onTriggered: page.signal(menu.row, "term")
        }
        QQC2.MenuItem {
            text: qsTr("Kill task")
            icon.source: Icons.of("kill")
            icon.color: Kirigami.Theme.textColor
            onTriggered: page.signal(menu.row, "kill")
        }
        QQC2.MenuItem {
            readonly property bool suspended: menu.row?.state === "T"
            text: suspended ? qsTr("Resume") : qsTr("Suspend")
            icon.source: Icons.of(suspended ? "resume" : "suspend")
            icon.color: Kirigami.Theme.textColor
            onTriggered: page.signal(menu.row, suspended ? "cont" : "stop")
        }
        QQC2.MenuSeparator {}
        QQC2.Menu {
            title: qsTr("Priority")
            Repeater {
                model: page.priorities
                QQC2.MenuItem {
                    required property var modelData
                    text: modelData[1]
                    checkable: true
                    checked: menu.row?.nice === modelData[0]
                    onTriggered: page.setPriority(menu.row, modelData[0])
                }
            }
        }
        QQC2.MenuSeparator {}
        QQC2.MenuItem {
            text: qsTr("Open file location")
            icon.source: Icons.of("open-folder")
            icon.color: Kirigami.Theme.textColor
            enabled: !!menu.row?.exe
            onTriggered: Qt.openUrlExternally("file://" + page.folder(menu.row.exe))
        }
        QQC2.MenuItem {
            text: qsTr("Copy command line")
            icon.source: Icons.of("copy")
            icon.color: Kirigami.Theme.textColor
            onTriggered: {
                clipboard.text = menu.row.command
                clipboard.selectAll()
                clipboard.copy()
            }
        }
    }

    TextEdit {
        id: clipboard
        visible: false
    }
}
