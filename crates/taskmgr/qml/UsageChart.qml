import QtQuick
import org.kde.kirigami as Kirigami
import org.kde.quickcharts as Charts

// Line chart over the last 60 samples, filled like the Windows graphs.
// `series` is a list of up to two { values: [...], color: ... }.
Rectangle {
    id: root

    property var series: []
    property real maximum: 100
    property color accent: Kirigami.Theme.highlightColor
    // Small version used in the resource list: no grid.
    property bool compact: false

    // Samples fill the chart from the right while the history is short.
    readonly property int sampleCount: Math.max(1, ...series.map(s => s.values.length))

    color: Qt.alpha(accent, 0.04)
    border.color: Qt.alpha(accent, compact ? 0.8 : 0.6)
    border.width: 1

    Repeater {
        model: root.compact ? 0 : 9
        Rectangle {
            required property int index
            x: 0
            y: Math.round((index + 1) * root.height / 10)
            width: root.width
            height: 1
            color: Qt.alpha(root.accent, 0.12)
        }
    }

    Repeater {
        model: root.compact ? 0 : 19
        Rectangle {
            required property int index
            x: Math.round((index + 1) * root.width / 20)
            y: 0
            width: 1
            height: root.height
            color: Qt.alpha(root.accent, 0.12)
        }
    }

    // A short history occupies the right part only, as time flows leftwards.
    Charts.LineChart {
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.margins: 1
        width: (parent.width - 2) * Math.max(0, root.sampleCount - 1) / 59

        xRange {
            from: 0
            to: Math.max(1, root.sampleCount - 1)
            automatic: false
        }
        yRange {
            from: 0
            to: root.maximum
            automatic: false
        }

        lineWidth: root.compact ? 1 : 1.5
        fillOpacity: 0.15
        interpolate: false

        // At most two series (read/write, received/sent).
        valueSources: root.series.length > 1 ? [first, second] : [first]
        colorSource: Charts.ArraySource { array: root.series.map(s => s.color) }

        Charts.ArraySource {
            id: first
            array: root.series.length > 0 ? root.series[0].values : []
        }
        Charts.ArraySource {
            id: second
            array: root.series.length > 1 ? root.series[1].values : []
        }
    }
}
