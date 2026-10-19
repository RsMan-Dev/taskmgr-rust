import QtQuick
import org.kde.kirigami as Kirigami
import org.taskmgr.charts

// Line chart over the last 60 samples, filled like the Windows graphs.
// `series` is a list of up to two { values: [...], color: ... }.
Rectangle {
    id: root

    property var series: []
    property real maximum: 100
    property color accent: Kirigami.Theme.highlightColor
    // Small version used in the resource list: no grid.
    property bool compact: false


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

    SeriesPlot {
        anchors.fill: parent
        anchors.margins: 1
        series: root.series
        maximum: root.maximum
        lineWidth: root.compact ? 1 : 1.5
        capacity: Prefs.historyPoints
        smooth: Prefs.smoothGraphs
    }
}
