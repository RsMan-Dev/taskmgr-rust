import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// TCP and UDP sockets with the application that owns them.
ColumnLayout {
    id: page

    required property var monitor
    property var groups: null
    property bool active: false
    // 0: all, 1: connected, 2: listening.
    property int filter: 0

    signal showProcess(int pid)

    readonly property real unit: Kirigami.Units.gridUnit
    readonly property var owners: {
        const map = {}
        for (const g of groups ?? []) {
            for (const p of g.processes) {
                map[p.pid] = g.name
            }
        }
        return map
    }
    readonly property var connections: {
        const all = monitor.connections !== "" ? JSON.parse(monitor.connections) : []
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
        return c.pid === null ? "" : Tr.core(owners[c.pid] ?? "") || qsTr("PID %1").arg(c.pid)
    }

    function endpoint(address, port) {
        if (address === "") {
            return ""
        }
        return (address.includes(":") ? "[" + address + "]" : address) + ":" + port
    }

    function stateName(c) {
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

    DataTable {
        rows: page.connections
        keyOf: c => [c.protocol, c.local, c.local_port, c.remote, c.remote_port].join(" ")
        sortColumn: "app"
        placeholder: page.monitor.connections === "" ? "" : qsTr("No matching connection")
        Layout.fillWidth: true
        Layout.fillHeight: true
        columns: [
            { key: "app", title: qsTr("Application"), maxWidth: page.unit * 14, text: c => page.appName(c),
              value: c => page.appName(c) || "￿", dim: c => c.pid === null },
            { key: "pid", title: qsTr("PID"), align: Text.AlignRight, text: c => c.pid ?? "", value: c => c.pid ?? -1 },
            { key: "protocol", title: qsTr("Protocol"), text: c => c.protocol + (c.ipv6 ? "v6" : "") },
            { key: "local", title: qsTr("Local address"), text: c => page.endpoint(c.local, c.local_port) },
            { key: "remote", title: qsTr("Remote address"), text: c => page.endpoint(c.remote, c.remote_port) },
            { key: "state", title: qsTr("State"), text: c => page.stateName(c) },
            { key: "user", title: qsTr("User"), width: 0, text: c => c.user },
        ]
        onActivated: (c) => {
            if (c.pid !== null) {
                page.showProcess(c.pid)
            }
        }
    }
}
