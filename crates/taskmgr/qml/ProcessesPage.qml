import QtCore
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: page

    required property var groups
    property var snapshot
    required property var monitor

    property string sortColumn: "name"
    property bool sortDescending: false
    property var expanded: ({})
    property string selectedKey: ""
    property string error: ""

    readonly property real columnWidth: Kirigami.Units.gridUnit * 6.5
    readonly property real nameMinimumWidth: Kirigami.Units.gridUnit * 14
    readonly property real viewportWidth: list.width - (list.QQC2.ScrollBar.vertical.visible ? list.QQC2.ScrollBar.vertical.width : 0)
    // Names shrink down to their minimum, then the table scrolls sideways.
    readonly property real tableWidth: Math.max(viewportWidth, nameMinimumWidth + columns.length * columnWidth)
    readonly property int logical: snapshot?.cpu.logical ?? 1
    readonly property var dedicatedGpus: snapshot ? snapshot.gpus.filter(g => g.dedicated) : []

    // Every metric column: key, header, header total, value and cell text.
    readonly property var allColumns: {
        const factor = prefs.cpuPerCore ? logical : 1
        const s = snapshot
        const cols = [
            { key: "cpu", title: "Processeur", total: s ? Format.percent(s.cpu.usage * factor) : "",
              value: x => x.cpu * factor, reference: 25 * factor, text: v => Format.num(v, 1) + " %" },
            { key: "memory", title: "Mémoire", total: s ? Format.percent(s.memory.usage) : "",
              value: x => x.memory, reference: 2 * 1024 * 1024 * 1024, text: v => Format.bytes(v) },
            { key: "disk", title: "Disque",
              total: s ? Format.num(s.disks.reduce((a, d) => a + d.read_bps + d.write_bps, 0) / 1048576, 1) + " Mo/s" : "",
              value: x => x.disk_bps ?? -1, reference: 20 * 1024 * 1024,
              text: v => v < 0 ? "" : Format.num(v / 1048576, 1) + " Mo/s" },
        ]
        for (const g of dedicatedGpus) {
            cols.push({ key: "gpu" + g.index, title: "GPU " + g.index, tip: g.name, total: Format.percent(g.usage),
                        value: x => x.gpu?.[g.index] ?? 0, reference: 50, text: v => Format.num(v, 0) + " %" })
            cols.push({ key: "gpumem" + g.index, title: "Mém. GPU " + g.index, tip: g.name,
                        total: g.memory_total ? Format.percent(g.memory_used / g.memory_total * 100) : "",
                        value: x => x.gpu_memory?.[g.index] ?? 0, reference: 4 * 1024 * 1024 * 1024, text: v => v > 0 ? Format.bytes(v) : "" })
        }
        cols.push({ key: "threads", title: "Threads", total: s ? Format.num(s.cpu.threads, 0) : "",
                    value: x => x.threads ?? (x.processes ?? []).reduce((a, p) => a + p.threads, 0), reference: 1e9, text: v => Format.num(v, 0) })
        return cols
    }
    readonly property var columns: allColumns.filter(c => !prefs.hiddenList.includes(c.key))

    Settings {
        id: prefs
        category: "processes"
        property string hidden: "threads,gpumem1,gpumem0,gpumem2"
        property bool cpuPerCore: false
        readonly property var hiddenList: hidden.split(",")

        function toggle(key) {
            const list = hiddenList.filter(k => k !== "" && k !== key)
            hidden = (hiddenList.includes(key) ? list : list.concat([key])).join(",")
        }
    }
    readonly property var sectionTitles: [
        ["app", "Applications"],
        ["background", "Processus en arrière-plan"],
        ["system", "Processus système"],
    ]

    readonly property var table: build(groups, search.text.trim().toLowerCase(), sortColumn, sortDescending, expanded)
    readonly property var selected: table.data[selectedKey] ?? null

    spacing: 0

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

    // Moves rows in place so delegates and the scroll position survive updates.
    function sync(keys) {
        for (let i = 0; i < keys.length; i++) {
            if (i < rows.count && rows.get(i).key === keys[i]) {
                continue
            }
            let j = -1
            for (let k = i + 1; k < rows.count; k++) {
                if (rows.get(k).key === keys[i]) {
                    j = k
                    break
                }
            }
            if (j >= 0) {
                rows.move(j, i, 1)
            } else {
                rows.insert(i, { key: keys[i] })
            }
        }
        if (rows.count > keys.length) {
            rows.remove(keys.length, rows.count - keys.length)
        }
    }

    onTableChanged: sync(table.keys)

    function setExpanded(groupKey, open) {
        const e = Object.assign({}, expanded)
        if (open) {
            e[groupKey] = true
        } else {
            delete e[groupKey]
        }
        expanded = e
    }

    function pidsOf(entry) {
        return entry.type === "group" ? entry.group.processes.map(p => p.pid) : [entry.proc.pid]
    }

    function endTask(entry, force) {
        if (!entry || entry.type === "section") {
            return
        }
        error = monitor.terminate(pidsOf(entry).join(","), force)
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
        title: "Processus"

        Kirigami.SearchField {
            id: search
            placeholderText: "Rechercher un nom ou un PID"
            Layout.fillWidth: true
            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
            Layout.maximumWidth: Kirigami.Units.gridUnit * 16
            Layout.minimumWidth: Kirigami.Units.gridUnit * 6
        }
        QQC2.Button {
            display: page.width < Kirigami.Units.gridUnit * 30 ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered && display === QQC2.AbstractButton.IconOnly
            text: "Fin de tâche"
            icon.name: "process-stop"
            enabled: page.selected !== null && page.selected.type !== "section"
            onClicked: page.endTask(page.selected, false)
        }
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

        HeaderCell {
            column: "name"
            title: "Nom"
            alignment: Qt.AlignLeft
            Layout.fillWidth: true
        }
        Repeater {
            model: page.columns
            HeaderCell {
                required property var modelData
                column: modelData.key
                title: modelData.title
                total: modelData.total
                tip: modelData.tip ?? ""
                Layout.preferredWidth: page.columnWidth
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

        QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
        QQC2.ScrollBar.horizontal: QQC2.ScrollBar {}

        model: ListModel { id: rows }

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
        Keys.onDeletePressed: page.endTask(page.selected, false)

        delegate: Loader {
            id: slot
            required property string key
            readonly property var entry: page.table.data[key] ?? null
            width: page.tableWidth
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
                    columnWidth: page.columnWidth
                    selected: page.selectedKey === slot.key
                    expanded: entry.type === "group" && !!page.expanded[entry.group.key]
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
            icon.name: "edit-none"
            text: "Aucun processus ne correspond"
        }

        Kirigami.LoadingPlaceholder {
            anchors.centerIn: parent
            visible: page.groups === null
        }
    }

    QQC2.Menu {
        id: menu
        property var entry: null

        QQC2.MenuItem {
            text: "Fin de tâche"
            icon.name: "process-stop"
            onTriggered: page.endTask(menu.entry, false)
        }
        QQC2.MenuItem {
            text: "Forcer l'arrêt"
            icon.name: "edit-delete"
            onTriggered: page.endTask(menu.entry, true)
        }
        QQC2.MenuSeparator {}
        QQC2.MenuItem {
            visible: menu.entry?.type === "group" && menu.entry.group.processes.length > 1
            height: visible ? implicitHeight : 0
            text: menu.entry?.type === "group" && page.expanded[menu.entry.group.key] ? "Réduire" : "Développer"
            icon.name: "view-list-tree"
            onTriggered: page.setExpanded(menu.entry.group.key, !page.expanded[menu.entry.group.key])
        }
        QQC2.MenuItem {
            text: "Copier la ligne de commande"
            icon.name: "edit-copy"
            enabled: menu.entry?.type === "process" || menu.entry?.group?.processes.length === 1
            onTriggered: {
                const p = menu.entry.type === "process" ? menu.entry.proc : menu.entry.group.processes[0]
                clipboard.text = p.command
                clipboard.selectAll()
                clipboard.copy()
            }
        }
    }

    TextEdit {
        id: clipboard
        visible: false
    }

    QQC2.Menu {
        id: columnMenu

        Repeater {
            model: page.allColumns
            QQC2.MenuItem {
                required property var modelData
                text: modelData.title + (modelData.tip ? "  (" + modelData.tip + ")" : "")
                checkable: true
                checked: !prefs.hiddenList.includes(modelData.key)
                onTriggered: prefs.toggle(modelData.key)
            }
        }
        QQC2.MenuSeparator {}
        QQC2.MenuItem {
            text: "Processeur en % d'un cœur (comme top)"
            checkable: true
            checked: prefs.cpuPerCore
            onTriggered: prefs.cpuPerCore = !prefs.cpuPerCore
        }
    }

    component HeaderCell: QQC2.ItemDelegate {
        id: cell
        property string column
        property string title
        property string total: ""
        property string tip: ""
        property int alignment: Qt.AlignRight

        padding: 0
        implicitHeight: Kirigami.Units.gridUnit * 3
        onClicked: page.sortBy(column)
        QQC2.ToolTip.text: (tip ? tip + "\n" : "") + "Clic droit : choisir les colonnes"
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

        TapHandler {
            acceptedButtons: Qt.RightButton
            onTapped: columnMenu.popup()
        }
        background: Rectangle {
            color: cell.hovered ? Qt.alpha(Kirigami.Theme.highlightColor, 0.08) : "transparent"
        }

        contentItem: ColumnLayout {
            spacing: 0
            QQC2.Label {
                text: cell.total
                visible: cell.total !== ""
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.35
                font.features: { "tnum": 1 }
                Layout.alignment: cell.alignment
                Layout.leftMargin: Kirigami.Units.gridUnit
                Layout.rightMargin: Kirigami.Units.largeSpacing
            }
            Item {
                Layout.fillHeight: true
            }
            RowLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.alignment: cell.alignment
                Layout.leftMargin: Kirigami.Units.gridUnit
                Layout.rightMargin: Kirigami.Units.largeSpacing
                Layout.bottomMargin: Kirigami.Units.smallSpacing
                Kirigami.Icon {
                    visible: page.sortColumn === cell.column
                    source: page.sortDescending ? "arrow-down" : "arrow-up"
                    Layout.preferredWidth: Kirigami.Units.iconSizes.small
                    Layout.preferredHeight: Kirigami.Units.iconSizes.small
                }
                QQC2.Label {
                    text: cell.title
                    opacity: 0.75
                }
            }
        }
    }
}
