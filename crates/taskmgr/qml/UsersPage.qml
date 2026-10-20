import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Resources used by each user, summed over their processes.
ColumnLayout {
    id: page

    required property var groups
    property int logical: 1

    signal showUser(string user)

    readonly property var users: {
        const map = {}
        for (const g of groups ?? []) {
            for (const p of g.processes) {
                const u = map[p.user] ?? (map[p.user] = { user: p.user, count: 0, cpu: 0, memory: 0, disk: 0, net: -1 })
                u.count++
                u.cpu += p.cpu
                u.memory += p.memory
                u.disk += p.disk_bps ?? 0
                if (p.net) {
                    u.net = Math.max(u.net, 0) + p.net[0] + p.net[1]
                }
            }
        }
        return Object.values(map)
    }

    spacing: 0

    PageHeader {
        title: qsTr("Users")
    }

    DataTable {
        rows: page.users
        keyOf: row => row.user
        sortColumn: "memory"
        sortDescending: true
        Layout.fillWidth: true
        Layout.fillHeight: true
        columns: [
            { key: "user", title: qsTr("User"), text: r => r.user, width: 0 },
            { key: "count", title: qsTr("Processes"), value: r => r.count, text: r => r.count, align: Qt.AlignRight, widest: ["99999"] },
            { key: "cpu", title: qsTr("CPU"), value: r => r.cpu, align: Qt.AlignRight,
              text: r => Format.percent(r.cpu * (Prefs.cpuPerCore ? page.logical : 1), 1),
              widest: [Format.percent(100 * (Prefs.cpuPerCore ? page.logical : 1), 1)] },
            { key: "memory", title: qsTr("Memory"), value: r => r.memory, text: r => Format.bytes(r.memory), align: Qt.AlignRight,
              widest: [Format.bytes(999 * 1048576), Format.bytes(99.9 * 1073741824)] },
            { key: "disk", title: qsTr("Disk"), value: r => r.disk, text: r => Format.rate(r.disk), align: Qt.AlignRight,
              widest: [Format.rate(999 * 1048576)] },
            { key: "net", title: qsTr("Network"), value: r => r.net, align: Qt.AlignRight,
              text: r => r.net < 0 ? "" : Format.bits(r.net), widest: [Format.bits(999e6)] },
        ]
        onActivated: (row) => page.showUser(row.user)
    }
}
