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
    readonly property var sectionTitles: [
        ["app", "Applications"],
        ["background", "Processus en arrière-plan"],
        ["system", "Processus système"],
    ]

    readonly property var table: build(groups, search.text.trim().toLowerCase(), sortColumn, sortDescending, expanded)
    readonly property var selected: table.data[selectedKey] ?? null

    spacing: 0

    function compare(column, descending) {
        const value = x => column === "name" ? 0 : column === "disk" ? (x.disk_bps ?? -1) : x[column]
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
            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
        }
        QQC2.Button {
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

    RowLayout {
        spacing: 0
        Layout.fillWidth: true
        Layout.rightMargin: list.QQC2.ScrollBar.vertical.visible ? list.QQC2.ScrollBar.vertical.width : 0

        HeaderCell {
            column: "name"
            title: "Nom"
            alignment: Qt.AlignLeft
            Layout.fillWidth: true
        }
        HeaderCell {
            column: "cpu"
            title: "Processeur"
            total: page.snapshot ? Format.percent(page.snapshot.cpu.usage) : ""
            Layout.preferredWidth: page.columnWidth
        }
        HeaderCell {
            column: "memory"
            title: "Mémoire"
            total: page.snapshot ? Format.percent(page.snapshot.memory.usage) : ""
            Layout.preferredWidth: page.columnWidth
        }
        HeaderCell {
            column: "disk"
            title: "Disque"
            total: page.snapshot && page.snapshot.disks.length > 0
                ? Format.percent(Math.max(...page.snapshot.disks.map(d => d.active))) : ""
            Layout.preferredWidth: page.columnWidth
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

        QQC2.ScrollBar.vertical: QQC2.ScrollBar {}

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
            width: ListView.view.width - (list.QQC2.ScrollBar.vertical.visible ? list.QQC2.ScrollBar.vertical.width : 0)
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

    component HeaderCell: QQC2.ItemDelegate {
        id: cell
        property string column
        property string title
        property string total: ""
        property int alignment: Qt.AlignRight

        padding: 0
        implicitHeight: Kirigami.Units.gridUnit * 3
        onClicked: page.sortBy(column)
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
