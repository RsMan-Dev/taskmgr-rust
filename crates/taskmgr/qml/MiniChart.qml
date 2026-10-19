import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Secondary metric under a main graph: caption, current value, small graph.
ColumnLayout {
    id: mini

    property string label
    property string value
    property var values: []
    property real maximum: 100
    property color accent: Kirigami.Theme.highlightColor
    // Shown instead of the graph when the metric cannot be read.
    property string unavailable: ""

    spacing: Kirigami.Units.smallSpacing / 2

    RowLayout {
        QQC2.Label {
            text: mini.label
            opacity: 0.7
            font: Kirigami.Theme.smallFont
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
        QQC2.Label {
            text: mini.unavailable === "" ? mini.value : ""
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            font.weight: Font.DemiBold
            font.features: { "tnum": 1 }
        }
    }

    UsageChart {
        visible: mini.unavailable === ""
        series: [{ values: mini.values, color: mini.accent }]
        maximum: Math.max(mini.maximum, ...mini.values, 1)
        accent: mini.accent
        compact: true
        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 3.2
    }

    Rectangle {
        visible: mini.unavailable !== ""
        color: "transparent"
        border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
        radius: Kirigami.Units.cornerRadius
        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 3.2

        QQC2.Label {
            anchors.fill: parent
            anchors.margins: Kirigami.Units.smallSpacing
            text: mini.unavailable
            wrapMode: Text.Wrap
            opacity: 0.6
            font: Kirigami.Theme.smallFont
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }
}
