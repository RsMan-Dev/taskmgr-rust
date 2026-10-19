import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: page

    required property var groups
    property var snapshot
    required property var monitor
    property var processDetails: ({})

    property string sortColumn: Prefs.sortColumn
    property bool sortDescending: Prefs.sortDescending
    onSortColumnChanged: Prefs.sortColumn = sortColumn
    onSortDescendingChanged: Prefs.sortDescending = sortDescending
    property var expanded: ({})

    signal journalRequested(string title, var query)
    property string selectedKey: ""
    property string error: ""

    readonly property real nameMinimumWidth: Kirigami.Units.gridUnit * 14
    readonly property real viewportWidth: list.width - (list.QQC2.ScrollBar.vertical.visible ? list.QQC2.ScrollBar.vertical.width : 0)
    // Names shrink down to their minimum, then the table scrolls sideways.
    readonly property real tableWidth: Math.max(viewportWidth, nameMinimumWidth + columns.reduce((a, c) => a + c.width, 0))
    readonly property var allColumns: columnSet.allColumns
    readonly property var columns: columnSet.columns

    ProcessColumns {
        id: columnSet
        snapshot: page.snapshot
    }

    readonly property var sectionTitles: [
        ["app", qsTr("Apps")],
        ["background", qsTr("Background processes")],
        ["system", qsTr("System processes")],
    ]

    readonly property var table: build(groups, search.text.trim().toLowerCase(), sortColumn, sortDescending, expanded)
    readonly property var selected: table.data[selectedKey] ?? null

    spacing: 0

    function focusSearch() {
        search.forceActiveFocus()
        search.selectAll()
    }

    function compare(column, descending) {
        const def = allColumns.find(c => c.key === column)
        const value = x => def ? def.value(x) : 0
        return (a, b) => {
            const r = column === "name"
                ? a.name.localeCompare(b.name, Qt.locale().name, { sensitivity: "base" })
                : value(a) - value(b) || b.name.localeCompare(a.name)
            return descending ? -r : r
        }
    }

    function build(groups, query, column, descending, expanded) {
        const keys = []
        const data = {}
        if (!groups) {
            return { keys: keys, data: data }
        }
        const cmp = compare(column, descending)
        const roleRank = { normal: 0, infra: 1, wrapper: 2 }
        const matches = p => p.name.toLowerCase().includes(query) || String(p.pid) === query
        for (const [kind, title] of sectionTitles) {
            let list = groups.filter(g => g.kind === kind)
            if (query !== "") {
                list = list.filter(g => g.name.toLowerCase().includes(query) || g.processes.some(matches))
            }
            if (list.length === 0) {
                continue
            }
            keys.push("s:" + kind)
            data["s:" + kind] = { type: "section", title: title, count: list.length }
            list.sort(cmp)
            for (const g of list) {
                const gk = "g:" + g.key
                keys.push(gk)
                data[gk] = { type: "group", group: g }
                const open = expanded[g.key] || (query !== "" && g.processes.some(matches))
                if (!open || g.processes.length < 2) {
                    continue
                }
                const procs = g.processes.slice().sort((a, b) => roleRank[a.role] - roleRank[b.role] || cmp(a, b))
                for (const p of procs) {
                    const pk = "p:" + g.key + ":" + p.pid
                    keys.push(pk)
                    data[pk] = { type: "process", proc: p, group: g }
                }
            }
        }
        return { keys: keys, data: data }
    }

    onTableChanged: rows.sync(table.keys)

    function setExpanded(groupKey, open) {
        const e = Object.assign({}, expanded)
        if (open) {
            e[groupKey] = true
        } else {
            delete e[groupKey]
        }
        expanded = e
    }

    // "pid:start" pairs: actions skip a process whose pid was reused.
    function targetsOf(entry) {
        const list = entry.type === "group" ? entry.group.processes : [entry.proc]
        return list.map(p => p.pid + ":" + p.start).join(",")
    }

    function sendSignal(entry, sig) {
        if (!entry || entry.type === "section") {
            return
        }
        error = monitor.sendSignal(targetsOf(entry), sig) === "denied"
            ? qsTr("Access denied: this process belongs to another user or to the system.") : ""
    }

    function sortBy(column) {
        if (sortColumn === column) {
            sortDescending = !sortDescending
        } else {
            sortColumn = column
            sortDescending = column !== "name"
        }
    }

    function moveSelection(delta) {
        const keys = table.keys.filter(k => !k.startsWith("s:"))
        if (keys.length === 0) {
            return
        }
        const i = keys.indexOf(selectedKey)
        const next = keys[Math.max(0, Math.min(keys.length - 1, i < 0 ? 0 : i + delta))]
        selectedKey = next
        list.positionViewAtIndex(table.keys.indexOf(next), ListView.Contain)
    }

    PageHeader {
        title: qsTr("Processes")

        Kirigami.SearchField {
            id: search
            placeholderText: qsTr("Search a name or PID")
            Layout.fillWidth: true
            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
            Layout.maximumWidth: Kirigami.Units.gridUnit * 16
            Layout.minimumWidth: Kirigami.Units.gridUnit * 6
        }
        QQC2.Button {
            display: page.width < Kirigami.Units.gridUnit * 36 ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
            text: qsTr("End task")
            icon.source: Icons.of("end-task", Kirigami.Theme.textColor)
            enabled: page.selected !== null && page.selected.type !== "section"
            onClicked: page.sendSignal(page.selected, "term")
            QQC2.ToolTip.text: qsTr("Asks the process to quit (SIGTERM)")
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }
        QQC2.Button {
            display: page.width < Kirigami.Units.gridUnit * 36 ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
            text: qsTr("Kill")
            icon.source: Icons.of("kill", Kirigami.Theme.textColor)
            enabled: page.selected !== null && page.selected.type !== "section"
            onClicked: page.sendSignal(page.selected, "kill")
            QQC2.ToolTip.text: qsTr("Stops the process at once, without giving it time to save (SIGKILL)")
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        }
    }

    Kirigami.InlineMessage {
        readonly property string access: page.monitor.networkAccess
        readonly property bool failed: access !== "" && access !== "pending" && access !== "active"
        visible: !Prefs.isHidden("net") && access !== "active"
        type: failed ? Kirigami.MessageType.Error : Kirigami.MessageType.Information
        text: access === "pending" ? qsTr("Waiting for the administrator password…")
            : failed ? Tr.core(access)
            : qsTr("The Network column measures each process's traffic and needs administrator rights.")
        actions: [
            Kirigami.Action {
                text: qsTr("Enable…")
                icon.source: Icons.of("admin", Kirigami.Theme.textColor)
                enabled: page.monitor.networkAccess !== "pending"
                onTriggered: page.monitor.enableNetworkMonitor()
            },
            Kirigami.Action {
                text: qsTr("Hide the column")
                icon.source: Icons.of("hide", Kirigami.Theme.textColor)
                onTriggered: Prefs.toggleColumn("net")
            }
        ]
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.gridUnit
        Layout.rightMargin: Kirigami.Units.gridUnit
        Layout.bottomMargin: Kirigami.Units.smallSpacing
    }

    Kirigami.InlineMessage {
        visible: page.error !== ""
        text: page.error
        type: Kirigami.MessageType.Error
        showCloseButton: true
        onVisibleChanged: if (!visible) page.error = ""
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.gridUnit
        Layout.rightMargin: Kirigami.Units.gridUnit
        Layout.bottomMargin: Kirigami.Units.smallSpacing
    }

    Item {
        clip: true
        implicitHeight: header.implicitHeight
        Layout.fillWidth: true

        RowLayout {
            id: header
            x: -list.contentX
            width: page.tableWidth
            spacing: 0

            ProcessHeaderCell {
                column: "name"
                title: qsTr("Name")
                alignment: Qt.AlignLeft
                sorted: page.sortColumn === column
                descending: page.sortDescending
                onSortRequested: page.sortBy(column)
                onColumnsMenuRequested: columnMenu.popup()
                Layout.fillWidth: true
            }
            Repeater {
                model: page.columns
                ProcessHeaderCell {
                    required property var modelData
                    column: modelData.key
                    title: modelData.title
                    total: columnSet.totals[modelData.key] ?? ""
                    tip: modelData.tip ?? ""
                    sorted: page.sortColumn === column
                    descending: page.sortDescending
                    onSortRequested: page.sortBy(column)
                    onColumnsMenuRequested: columnMenu.popup()
                    Layout.preferredWidth: modelData.width
                }
            }
        }
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    ListView {
        id: list
        clip: true
        focus: true
        activeFocusOnTab: true
        boundsBehavior: Flickable.StopAtBounds
        reuseItems: true
        Layout.fillWidth: true
        Layout.fillHeight: true
        contentWidth: page.tableWidth
        flickableDirection: Flickable.AutoFlickIfNeeded

        QQC2.ScrollBar.vertical: ThinScrollBar {}
        QQC2.ScrollBar.horizontal: ThinScrollBar {}

        model: KeyedModel { id: rows }

        Keys.onUpPressed: page.moveSelection(-1)
        Keys.onDownPressed: page.moveSelection(1)
        Keys.onRightPressed: if (page.selected?.type === "group") page.setExpanded(page.selected.group.key, true)
        Keys.onLeftPressed: {
            const s = page.selected
            if (s?.type === "group") {
                page.setExpanded(s.group.key, false)
            } else if (s?.type === "process") {
                page.selectedKey = "g:" + s.group.key
                page.setExpanded(s.group.key, false)
            }
        }
        Keys.onDeletePressed: page.sendSignal(page.selected, "term")

        delegate: Loader {
            id: slot
            required property string key
            readonly property var entry: page.table.data[key] ?? null
            width: page.tableWidth
            active: entry !== null
            sourceComponent: entry?.type === "section" ? sectionComponent : rowComponent

            Component {
                id: sectionComponent
                RowLayout {
                    height: Kirigami.Units.gridUnit * 2.4
                    Kirigami.Heading {
                        level: 4
                        text: slot.entry.title + "  (" + slot.entry.count + ")"
                        Layout.leftMargin: Kirigami.Units.gridUnit
                        Layout.alignment: Qt.AlignBottom
                        Layout.bottomMargin: Kirigami.Units.smallSpacing
                    }
                }
            }

            Component {
                id: rowComponent
                ProcessRow {
                    entry: slot.entry
                    columns: page.columns
                    height: Kirigami.Units.gridUnit * 1.9
                    selected: page.selectedKey === slot.key
                    expanded: entry?.type === "group" && !!page.expanded[entry.group.key]
                    onClicked: {
                        page.selectedKey = slot.key
                        list.forceActiveFocus()
                    }
                    onToggleRequested: page.setExpanded(entry.group.key, !expanded)
                    onMenuRequested: (position) => {
                        page.selectedKey = slot.key
                        menu.entry = entry
                        menu.popup(this, position)
                    }
                }
            }
        }

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: page.groups !== null && list.count === 0
            icon.source: Icons.of("search", Kirigami.Theme.textColor)
            text: qsTr("No matching process")
        }

        Kirigami.LoadingPlaceholder {
            anchors.centerIn: parent
            visible: page.groups === null
        }
    }

    ProcessMenu {
        id: menu
        expanded: entry?.type === "group" && !!page.expanded[entry.group.key]
        processDetails: page.processDetails
        onSignalRequested: (entry, sig) => page.sendSignal(entry, sig)
        onToggleRequested: (key) => page.setExpanded(key, !page.expanded[key])
        onJournalRequested: (title, query) => page.journalRequested(title, query)
    }

    QQC2.Menu {
        id: columnMenu

        Repeater {
            model: page.allColumns
            QQC2.MenuItem {
                required property var modelData
                text: modelData.title + (modelData.tip ? "  (" + modelData.tip + ")" : "")
                checkable: true
                checked: !Prefs.isHidden(modelData.key)
                onTriggered: Prefs.toggleColumn(modelData.key)
            }
        }
        QQC2.MenuSeparator {}
        QQC2.MenuItem {
            text: qsTr("CPU as % of one core (like top)")
            checkable: true
            checked: Prefs.cpuPerCore
            onTriggered: Prefs.cpuPerCore = !Prefs.cpuPerCore
        }
    }
}
