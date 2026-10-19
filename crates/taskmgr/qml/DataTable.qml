import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Flat sortable table. `columns`: { key, title, width (0: fills, absent:
// fits the content), text(row), value(row) for sorting, align, dim(row),
// widest: sample texts for numbers }. `rows`: plain objects.
// A column with `guides` draws tree lines from row.depth and row.guide
// ({ through: [bool per depth], last }), one `indentUnit` per level.
// `groupBy(row)` folds rows sharing a key under a row built by
// `groupRow(key, rows)`, expanded with the chevron of the first column.
// A first column with `expander(row)` (true: open, false: closed, null:
// leaf) shows a chevron that emits `toggled(row)`: trees of any depth.
ColumnLayout {
    id: table

    property var columns: []
    property var rows: []
    property var keyOf: row => String(row.id)
    property string sortColumn: ""
    property bool sortDescending: false
    property string selectedKey: ""
    property string placeholder: ""
    // Keeps `rows` in the given order (tree view) instead of sorting.
    property bool ordered: false
    property var groupBy: null
    property var groupRow: null
    property var expanded: ({})
    // Shows every group open (while searching) without touching `expanded`.
    property bool expandAll: false
    // Groups start open; `expanded` then lists the ones the user closed.
    property bool openByDefault: false

    readonly property var selectedRow: index[selectedKey] ?? null
    property real indentUnit: Kirigami.Units.gridUnit
    readonly property real chevron: groupBy || columns[0]?.expander
        ? Kirigami.Units.iconSizes.small + Kirigami.Units.smallSpacing : 0
    readonly property real fillMinimum: Kirigami.Units.gridUnit * 12
    // Column widths only follow the content when rows come or go, or every
    // few refreshes: measuring every cell each second stalls the window.
    property var widths: ({})
    property int refreshes: 0
    property int measuredRows: -1
    onColumnsChanged: measure()
    onChevronChanged: measure()
    onSortedChanged: {
        const n = sorted?.length ?? 0
        if (slots.count < n) {
            const added = []
            for (let i = slots.count; i < n; i++) {
                added.push({})
            }
            slots.append(added)
        } else if (slots.count > n) {
            slots.remove(n, slots.count - n)
        }
        if (sorted.length !== measuredRows || ++refreshes % 5 === 0) {
            measure()
        }
    }

    function measure() {
        const out = {}
        const pad = Kirigami.Units.largeSpacing * 2
        for (const c of columns) {
            if (c.width !== undefined) {
                out[c.key] = c.width
                continue
            }
            const texts = c.widest ?? longest((sorted ?? []).map(r => String(c.text(r))))
            let w = metrics.advanceWidth(c.title) + Kirigami.Units.iconSizes.small + Kirigami.Units.smallSpacing
            for (const t of texts) {
                w = Math.max(w, metrics.advanceWidth(t))
            }
            const indent = c.key === columns[0].key ? chevron * 2 : 0
            out[c.key] = Math.min(Math.ceil(w) + pad + indent, c.maxWidth ?? Kirigami.Units.gridUnit * 18)
        }
        measuredRows = (sorted ?? []).length
        widths = out
    }
    readonly property real fixedWidth: columns.reduce((a, c) => a + (widths[c.key] || 0), 0)
    readonly property real viewportWidth: list.width - (list.QQC2.ScrollBar.vertical.visible ? list.QQC2.ScrollBar.vertical.width : 0)
    readonly property real tableWidth: Math.max(viewportWidth, fixedWidth + fillMinimum)
    readonly property real fillWidth: tableWidth - fixedWidth

    readonly property var sorted: {
        const col = columns.find(c => c.key === sortColumn)
        const order = list => {
            if (col && !ordered) {
                const v = col.value ?? col.text
                list.sort((a, b) => {
                    const x = v(a), y = v(b)
                    const r = typeof x === "number" && typeof y === "number"
                        ? x - y : String(x).localeCompare(String(y), Qt.locale().name, { sensitivity: "base" })
                    return sortDescending ? -r : r
                })
            }
            return list
        }
        if (!groupBy) {
            return order(rows.slice())
        }
        const buckets = new Map()
        for (const r of rows) {
            const k = groupBy(r)
            if (!buckets.has(k)) {
                buckets.set(k, [])
            }
            buckets.get(k).push(r)
        }
        const heads = []
        for (const [k, list] of buckets) {
            heads.push(list.length === 1 ? list[0] : Object.assign(groupRow(k, list), { _group: k, _rows: list }))
        }
        const out = []
        for (const h of order(heads)) {
            out.push(h)
            if (h._group !== undefined && isOpen(h._group)) {
                for (const r of order(h._rows.slice())) {
                    out.push(Object.assign({}, r, { _child: true }))
                }
            }
        }
        return out
    }
    readonly property var index: {
        const map = {}
        for (const r of sorted) {
            map[rowKey(r)] = r
        }
        return map
    }

    signal contextMenuRequested(var row, var item, point position)
    signal toggled(var row)
    signal activated(var row)

    spacing: 0

    // Measuring every cell on each refresh is the costly part of the
    // table: only the longest few texts can be the widest.
    function longest(texts) {
        const out = []
        for (const t of texts) {
            if (out.length < 4 || t.length > out[out.length - 1].length) {
                out.push(t)
                out.sort((a, b) => b.length - a.length)
                out.length = Math.min(out.length, 4)
            }
        }
        return out
    }

    function rowKey(r) {
        return r._group !== undefined ? "group:" + r._group : keyOf(r)
    }

    // Cells of a row, tab-separated, as pasted into a spreadsheet.
    function rowText(row) {
        return columns.map(c => String(c.text(row))).filter(t => t !== "").join("\t")
    }

    function copyRow(row) {
        clipboard.text = rowText(row)
        clipboard.selectAll()
        clipboard.copy()
    }

    TextEdit {
        id: clipboard
        visible: false
    }

    function isOpen(group) {
        return expandAll || openByDefault !== !!expanded[group]
    }

    function toggle(group) {
        const e = Object.assign({}, expanded)
        e[group] = !e[group]
        expanded = e
    }

    function sortBy(key) {
        if (sortColumn === key) {
            sortDescending = !sortDescending
        } else {
            sortColumn = key
            sortDescending = false
        }
    }

    function moveSelection(delta) {
        const keys = sorted.map(rowKey)
        if (keys.length === 0) {
            return
        }
        const i = keys.indexOf(selectedKey)
        const next = Math.max(0, Math.min(keys.length - 1, i < 0 ? 0 : i + delta))
        selectedKey = keys[next]
        list.positionViewAtIndex(next, ListView.Contain)
    }

    function cellWidth(c) {
        return widths[c.key] || table.fillWidth
    }

    FontMetrics {
        id: metrics
        font: Kirigami.Theme.defaultFont
    }

    Item {
        clip: true
        implicitHeight: header.implicitHeight
        Layout.fillWidth: true

        Row {
            id: header
            x: -list.contentX
            width: table.tableWidth

            Repeater {
                model: table.columns

                QQC2.ItemDelegate {
                    id: cell
                    required property var modelData
                    width: table.cellWidth(modelData)
                    padding: Kirigami.Units.largeSpacing
                    onClicked: table.sortBy(modelData.key)
                    background: Rectangle {
                        color: cell.hovered ? Qt.alpha(Kirigami.Theme.highlightColor, 0.08) : "transparent"
                    }
                    contentItem: RowLayout {
                        spacing: Kirigami.Units.smallSpacing
                        QQC2.Label {
                            text: cell.modelData.title
                            opacity: 0.75
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                            horizontalAlignment: cell.modelData.align === Qt.AlignRight ? Text.AlignRight : Text.AlignLeft
                        }
                        Kirigami.Icon {
                            visible: !table.ordered && table.sortColumn === cell.modelData.key
                            source: Icons.of(table.sortDescending ? "sort-down" : "sort-up", Kirigami.Theme.textColor)
                            Layout.preferredWidth: Kirigami.Units.iconSizes.small
                            Layout.preferredHeight: Kirigami.Units.iconSizes.small
                        }
                    }
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
        contentWidth: table.tableWidth
        flickableDirection: Flickable.AutoFlickIfNeeded
        Layout.fillWidth: true
        Layout.fillHeight: true

        QQC2.ScrollBar.vertical: ThinScrollBar {}
        QQC2.ScrollBar.horizontal: ThinScrollBar {}

        // Row n shows the n-th sorted entry: re-sorting only rebinds the
        // visible delegates, and rows that come or go are added or removed
        // at the end, so the view is never rebuilt.
        model: ListModel {
            id: slots
        }

        Keys.onUpPressed: table.moveSelection(-1)
        Keys.onDownPressed: table.moveSelection(1)
        Keys.onReturnPressed: if (table.selectedRow) table.activated(table.selectedRow)
        Keys.onPressed: (event) => {
            if (event.matches(StandardKey.Copy) && table.selectedRow) {
                table.copyRow(table.selectedRow)
                event.accepted = true
            }
        }

        delegate: QQC2.ItemDelegate {
            id: rowItem
            required property int index
            readonly property var row: table.sorted[index] ?? null
            readonly property string key: row ? table.rowKey(row) : ""
            width: table.tableWidth
            height: Kirigami.Units.gridUnit * 1.8
            padding: 0
            hoverEnabled: true
            background: Rectangle {
                color: table.selectedKey === rowItem.key ? Qt.alpha(Kirigami.Theme.highlightColor, 0.22)
                     : rowItem.hovered ? Qt.alpha(Kirigami.Theme.highlightColor, 0.08) : "transparent"
            }
            onClicked: {
                table.selectedKey = key
                list.forceActiveFocus()
            }
            onDoubleClicked: if (row) table.activated(row)

            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: (point) => {
                    table.selectedKey = rowItem.key
                    table.contextMenuRequested(rowItem.row, rowItem, point.position)
                }
            }
            TapHandler {
                acceptedDevices: PointerDevice.TouchScreen
                onLongPressed: {
                    table.selectedKey = rowItem.key
                    table.contextMenuRequested(rowItem.row, rowItem, point.position)
                }
            }

            contentItem: Row {
                Repeater {
                    model: table.columns
                    QQC2.Label {
                        id: cellLabel
                        required property var modelData
                        required property int index
                        readonly property bool first: index === 0
                        width: table.cellWidth(modelData)
                        height: rowItem.height
                        leftPadding: Kirigami.Units.largeSpacing + (rowItem.row && modelData.indent ? modelData.indent(rowItem.row) : 0)
                            + (first ? table.chevron * (rowItem.row?._child ? 2 : 1) : 0)
                        rightPadding: Kirigami.Units.largeSpacing
                        verticalAlignment: Text.AlignVCenter
                        horizontalAlignment: modelData.align === Qt.AlignRight ? Text.AlignRight : Text.AlignLeft
                        elide: Text.ElideRight
                        textFormat: Text.PlainText
                        font.features: { "tnum": 1 }
                        text: rowItem.row ? String(modelData.text(rowItem.row)) : ""
                        opacity: rowItem.row && modelData.dim && modelData.dim(rowItem.row) ? 0.55 : 1

                        // Tree guide lines.
                        Repeater {
                            model: cellLabel.modelData.guides && rowItem.row?.guide ? rowItem.row.depth : 0
                            Item {
                                required property int index
                                readonly property int level: index + 1
                                readonly property bool own: level === rowItem.row.depth
                                readonly property real lineX: Kirigami.Units.largeSpacing + (level - 1) * table.indentUnit + table.indentUnit / 2
                                readonly property color ink: Qt.alpha(Kirigami.Theme.textColor, 0.3)
                                anchors.fill: parent
                                Rectangle {
                                    visible: parent.own || rowItem.row.guide.through[parent.level]
                                    x: parent.lineX
                                    width: 1
                                    height: parent.own && rowItem.row.guide.last ? parent.height / 2 : parent.height
                                    color: parent.ink
                                }
                                Rectangle {
                                    visible: parent.own
                                    x: parent.lineX
                                    y: Math.round(parent.height / 2)
                                    width: table.indentUnit / 2 - 2
                                    height: 1
                                    color: parent.ink
                                }
                            }
                        }

                        Kirigami.Icon {
                            readonly property var group: rowItem.row?._group
                            readonly property var state: cellLabel.first && rowItem.row && cellLabel.modelData.expander
                                ? cellLabel.modelData.expander(rowItem.row) : null
                            readonly property bool open: group !== undefined ? table.isOpen(group) : state === true
                            visible: cellLabel.first && (group !== undefined || (state !== null && state !== undefined))
                            x: Kirigami.Units.largeSpacing + (rowItem.row && cellLabel.modelData.indent ? cellLabel.modelData.indent(rowItem.row) : 0)
                            anchors.verticalCenter: parent.verticalCenter
                            width: Kirigami.Units.iconSizes.small
                            height: width
                            source: Icons.of(open ? "collapse" : "expand", Kirigami.Theme.textColor)
                            TapHandler {
                                onTapped: parent.group !== undefined ? table.toggle(parent.group) : table.toggled(rowItem.row)
                            }
                        }
                    }
                }
            }
        }

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: list.count === 0 && table.placeholder !== ""
            text: table.placeholder
        }
    }
}
