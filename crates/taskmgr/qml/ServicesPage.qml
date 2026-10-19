import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// systemd services of the system and of the session.
ColumnLayout {
    id: page

    required property var monitor
    property bool active: false

    property string message: ""
    property bool busy: false
    property int scope: 0

    signal journalRequested(string title, var query)

    readonly property real unit: Kirigami.Units.gridUnit
    readonly property var services: {
        const all = page.monitor.services !== "" ? JSON.parse(page.monitor.services) : []
        const q = search.text.trim().toLowerCase()
        return all.filter(s => (scope === 0 || (scope === 1) !== s.user)
            && (q === "" || s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q)))
    }
    readonly property var selected: table.selectedRow

    spacing: 0

    function stateName(s) {
        switch (s) {
        case "active": return qsTr("Running")
        case "inactive": return qsTr("Stopped")
        case "failed": return qsTr("Failed")
        case "activating": return qsTr("Starting")
        case "deactivating": return qsTr("Stopping")
        case "reloading": return qsTr("Reloading")
        }
        return s
    }

    function startupName(s) {
        switch (s) {
        case "enabled": case "enabled-runtime": return qsTr("Enabled")
        case "disabled": return qsTr("Disabled")
        case "static": return qsTr("Static")
        case "masked": case "masked-runtime": return qsTr("Masked")
        case "indirect": return qsTr("Indirect")
        case "linked": case "linked-runtime": return qsTr("Linked")
        case "generated": case "": return qsTr("Generated")
        case "transient": return qsTr("Transient")
        }
        return s
    }

    function act(action) {
        if (!selected) {
            return
        }
        busy = true
        message = ""
        monitor.serviceAction(selected.name, selected.user, action)
    }

    onActiveChanged: if (active) monitor.refreshServices()

    Timer {
        interval: 5000
        repeat: true
        running: page.active && !page.busy
        onTriggered: page.monitor.refreshServices()
    }

    Connections {
        target: page.monitor
        function onActionResultChanged() {
            const r = JSON.parse(page.monitor.actionResult)
            if (r.action === "service") {
                page.busy = false
                page.message = r.ok ? "" : Tr.core(r.message)
            }
        }
    }

    PageHeader {
        title: qsTr("Services")

        QQC2.ComboBox {
            model: [qsTr("All"), qsTr("System"), qsTr("User")]
            currentIndex: page.scope
            onActivated: (i) => page.scope = i
        }
        Kirigami.SearchField {
            id: search
            placeholderText: qsTr("Search a service")
            Layout.fillWidth: true
            Layout.preferredWidth: page.unit * 14
            Layout.maximumWidth: page.unit * 14
            Layout.minimumWidth: page.unit * 6
        }
    }

    RowLayout {
        spacing: Kirigami.Units.smallSpacing
        Layout.leftMargin: page.unit
        Layout.rightMargin: page.unit
        Layout.bottomMargin: Kirigami.Units.smallSpacing

        QQC2.Button {
            text: qsTr("Start")
            icon.source: Icons.of("start", Kirigami.Theme.textColor)
            enabled: !page.busy && page.selected !== null && page.selected.active !== "active"
            onClicked: page.act("start")
        }
        QQC2.Button {
            text: qsTr("Stop")
            icon.source: Icons.of("stop", Kirigami.Theme.textColor)
            enabled: !page.busy && page.selected !== null && page.selected.active === "active"
            onClicked: page.act("stop")
        }
        QQC2.Button {
            text: qsTr("Restart")
            icon.source: Icons.of("refresh", Kirigami.Theme.textColor)
            enabled: !page.busy && page.selected !== null
            onClicked: page.act("restart")
        }
        Kirigami.Separator {
            Layout.preferredHeight: Kirigami.Units.gridUnit * 1.5
        }
        QQC2.Button {
            readonly property bool enabledAtBoot: page.selected?.file_state === "enabled"
            text: enabledAtBoot ? qsTr("Disable at startup") : qsTr("Enable at startup")
            icon.source: Icons.of(enabledAtBoot ? "disable" : "enable", Kirigami.Theme.textColor)
            enabled: !page.busy && page.selected !== null && ["enabled", "disabled"].includes(page.selected.file_state)
            onClicked: page.act(enabledAtBoot ? "disable" : "enable")
        }
        Kirigami.Separator {
            Layout.preferredHeight: Kirigami.Units.gridUnit * 1.5
        }
        QQC2.Button {
            text: qsTr("Journal")
            icon.source: Icons.of("journal", Kirigami.Theme.textColor)
            enabled: page.selected !== null
            onClicked: page.journalRequested(page.selected.name.replace(/\.service$/, ""),
                { unit: page.selected.name, user: page.selected.user, limit: 1000 })
        }
        QQC2.BusyIndicator {
            visible: page.busy
            running: visible
            Layout.preferredHeight: page.unit * 1.5
            Layout.preferredWidth: page.unit * 1.5
        }
        Item {
            Layout.fillWidth: true
        }
    }

    Kirigami.InlineMessage {
        visible: page.message !== ""
        text: page.message
        type: Kirigami.MessageType.Error
        showCloseButton: true
        onVisibleChanged: if (!visible) page.message = ""
        Layout.fillWidth: true
        Layout.leftMargin: page.unit
        Layout.rightMargin: page.unit
        Layout.bottomMargin: Kirigami.Units.smallSpacing
    }

    DataTable {
        id: table
        rows: page.services
        keyOf: row => (row.user ? "u:" : "s:") + row.name
        sortColumn: "name"
        placeholder: page.monitor.services === "" ? "" : qsTr("No matching service")
        Layout.fillWidth: true
        Layout.fillHeight: true
        columns: [
            { key: "name", title: qsTr("Name"), maxWidth: page.unit * 16, text: r => r.name.replace(/\.service$/, ""),
              dim: r => r.active !== "active" },
            { key: "state", title: qsTr("Status"), text: r => page.stateName(r.active) },
            { key: "startup", title: qsTr("Startup"), text: r => page.startupName(r.file_state) },
            { key: "scope", title: qsTr("Type"), text: r => r.user ? qsTr("User") : qsTr("System") },
            { key: "description", title: qsTr("Description"), width: 0, text: r => r.description },
        ]
        onActivated: (row) => page.act(row.active === "active" ? "restart" : "start")
    }
}
