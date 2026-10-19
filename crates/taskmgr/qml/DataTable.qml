import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Flat sortable table. `columns`: { key, title, width (0: fills, absent:
// fits the content), text(row), value(row) for sorting, align, dim(row),
// widest: sample texts for numbers }. `rows`: plain objects.
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

    readonly property var selectedRow: index[selectedKey] ?? null
    readonly property real fillMinimum: Kirigami.Units.gridUnit * 12
    readonly property var widths: {
        const out = {}
        const pad = Kirigami.Units.largeSpacing * 2
        for (const c of columns) {
            if (c.width !== undefined) {
                out[c.key] = c.width
                continue
            }
            const texts = new Set(c.widest ?? rows.map(r => String(c.text(r))))
            let w = metrics.advanceWidth(c.title) + Kirigami.Units.iconSizes.small + Kirigami.Units.smallSpacing
            for (const t of texts) {
                w = Math.max(w, metrics.advanceWidth(t))
            }
            out[c.key] = Math.min(Math.ceil(w) + pad, c.maxWidth ?? Kirigami.Units.gridUnit * 18)
        }
        return out
    }
    readonly property real fixedWidth: columns.reduce((a, c) => a + (widths[c.key] || 0), 0)
    readonly property real viewportWidth: list.width - (list.QQC2.ScrollBar.vertical.visible ? list.QQC2.ScrollBar.vertical.width : 0)
    readonly property real tableWidth: Math.max(viewportWidth, fixedWidth + fillMinimum)
    readonly property real fillWidth: tableWidth - fixedWidth

    readonly property var sorted: {
        const col = columns.find(c => c.key === sortColumn)
        const list = rows.slice()
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
    readonly property var index: {
        const map = {}
        for (const r of sorted) {
            map[keyOf(r)] = r
        }
        return map
    }

    signal contextMenuRequested(var row, var item, point position)
    signal activated(var row)

    spacing: 0
    onSortedChanged: model.sync(sorted.map(keyOf))

    function sortBy(key) {
        if (sortColumn === key) {
            sortDescending = !sortDescending
        } else {
            sortColumn = key
            sortDescending = false
        }
    }

    function moveSelection(delta) {
        const keys = sorted.map(keyOf)
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

        QQC2.ScrollBar.vertical: QQC2.ScrollBar {}
        QQC2.ScrollBar.horizontal: QQC2.ScrollBar {}

        model: KeyedModel {
            id: model
        }

        Keys.onUpPressed: table.moveSelection(-1)
        Keys.onDownPressed: table.moveSelection(1)
        Keys.onReturnPressed: if (table.selectedRow) table.activated(table.selectedRow)

        delegate: QQC2.ItemDelegate {
            id: rowItem
            required property string key
            readonly property var row: table.index[key] ?? null
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

            contentItem: Row {
                Repeater {
                    model: table.columns
                    QQC2.Label {
                        required property var modelData
                        width: table.cellWidth(modelData)
                        height: rowItem.height
                        leftPadding: Kirigami.Units.largeSpacing + (rowItem.row && modelData.indent ? modelData.indent(rowItem.row) : 0)
                        rightPadding: Kirigami.Units.largeSpacing
                        verticalAlignment: Text.AlignVCenter
                        horizontalAlignment: modelData.align === Qt.AlignRight ? Text.AlignRight : Text.AlignLeft
                        elide: Text.ElideRight
                        textFormat: Text.PlainText
                        font.features: { "tnum": 1 }
                        text: rowItem.row ? String(modelData.text(rowItem.row)) : ""
                        opacity: rowItem.row && modelData.dim && modelData.dim(rowItem.row) ? 0.55 : 1
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
