import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// One small chart per processor (Windows' "logical processors" view).
ColumnLayout {
    id: block

    property var grid: null
    property var colors: ({})
    property real cellHeight: Kirigami.Units.gridUnit * 3

    signal menuRequested()

    visible: grid !== null
    spacing: Kirigami.Units.smallSpacing / 2

    RowLayout {
        Caption {
            text: block.grid?.caption ?? ""
            Layout.fillWidth: true
        }
        Caption {
            text: block.grid?.maximumLabel ?? ""
        }
    }
    GridLayout {
        id: coreGrid
        readonly property var lines: block.grid?.lines ?? []
        readonly property int count: lines.length
        columns: Math.max(1, Math.ceil(Math.sqrt(count * 2)))
        columnSpacing: Kirigami.Units.smallSpacing
        rowSpacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true

        Repeater {
            model: coreGrid.count
            UsageChart {
                required property int index
                readonly property string type: block.grid?.types?.[index] ?? ""
                compact: true
                series: [{ values: coreGrid.lines[index] ?? [], color: accent }].concat(block.grid?.kernel
                    ? [{ values: block.grid.kernel[index] ?? [], color: Qt.darker(accent, 1.6) }] : [])
                maximum: block.grid?.maximum ?? 100
                accent: type === "E" ? block.colors.efficiency : block.grid?.accent ?? block.colors.cpu
                Layout.fillWidth: true
                Layout.preferredHeight: Math.max(Kirigami.Units.gridUnit * 2.5, block.cellHeight / Math.ceil(coreGrid.count / coreGrid.columns))

                QQC2.ToolTip.text: block.grid?.numa ? qsTr("NUMA node %1").arg(index)
                    : qsTr("Processor %1").arg(index) + (type ? " (" + type + ")" : "")
                    + (block.grid?.accent === block.colors.clock ? " : " + Format.mhz((coreGrid.lines[index] ?? []).slice(-1)[0] ?? 0) : "")
                QQC2.ToolTip.visible: coreHover.hovered
                HoverHandler {
                    id: coreHover
                }
                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: block.menuRequested()
                }
                TapHandler {
                    acceptedDevices: PointerDevice.TouchScreen
                    onLongPressed: block.menuRequested()
                }
            }
        }
    }
}
