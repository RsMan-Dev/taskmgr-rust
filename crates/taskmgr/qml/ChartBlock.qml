import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Caption, chart and time axis. `chart` holds label, maximum,
// maximumLabel and series.
ColumnLayout {
    id: block
    property var chart
    property color accent
    property real chartHeight
    signal contextMenuRequested(point position)

    spacing: Kirigami.Units.smallSpacing / 2

    RowLayout {
        Caption {
            text: block.chart?.chartLabel ?? block.chart?.label ?? ""
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        Caption {
            text: block.chart?.maximumLabel ?? ""
        }
    }
    UsageChart {
        series: block.chart?.series ?? []
        maximum: block.chart?.maximum ?? 100
        accent: block.accent
        Layout.fillWidth: true
        Layout.preferredHeight: block.chartHeight

        TapHandler {
            acceptedButtons: Qt.RightButton
            onTapped: (point) => block.contextMenuRequested(point.position)
        }
    }
    RowLayout {
        Caption {
            text: Format.span(Prefs.historyPoints * Math.max(Prefs.intervalMs, 250) / 1000)
            Layout.fillWidth: true
        }
        Caption {
            text: "0"
        }
    }
}
