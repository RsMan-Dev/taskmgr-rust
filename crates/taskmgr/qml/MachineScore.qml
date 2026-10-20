import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// The machine score: geometric mean of the four test scores, so that one
// very fast part does not hide a slow one. 1000 is the reference machine.
Card {
    id: card

    required property var bench
    // Newest current-method run of each test, or undefined.
    property var cpu
    property var memory
    property var disk

    readonly property var parts: [
        [qsTr("Processor, one thread"), cpu?.cpu?.single],
        [qsTr("Processor, all threads"), cpu?.cpu?.multi],
        [qsTr("Memory"), memory?.points],
        [qsTr("Disk"), disk?.points],
    ]
    readonly property bool complete: parts.every(p => p[1] > 0)
    readonly property real overall: complete
        ? Math.exp(parts.reduce((a, p) => a + Math.log(p[1]), 0) / parts.length) : 0
    readonly property var steps: ["cpu", "memory", "disk"]
    readonly property bool runningAll: bench.running && !!bench.state?.all
    readonly property int step: steps.indexOf(bench.state?.kind ?? "")

    signal startAll()

    Layout.leftMargin: Kirigami.Units.gridUnit
    Layout.rightMargin: Kirigami.Units.gridUnit
    implicitHeight: column.implicitHeight + Kirigami.Units.gridUnit * 1.5

    ColumnLayout {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Kirigami.Units.gridUnit
        spacing: Kirigami.Units.smallSpacing

        RowLayout {
            Kirigami.Heading {
                text: qsTr("Machine score")
                level: 3
                Layout.fillWidth: true
            }
            QQC2.Label {
                text: card.complete ? Format.num(card.overall, 0) : "-"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 2
                font.weight: Font.DemiBold
            }
            QQC2.Button {
                text: card.runningAll ? qsTr("Stop") : qsTr("Run all")
                icon.source: Icons.of(card.runningAll ? "stop" : "start", Kirigami.Theme.textColor)
                enabled: card.runningAll || !card.bench.running
                onClicked: card.runningAll ? card.bench.monitor.stopBenchmark() : card.startAll()
                QQC2.ToolTip.text: qsTr("Runs the CPU, memory and disk tests one after the other (about two minutes).")
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
        }
        ProgressLine {
            visible: card.runningAll && card.step >= 0
            value: (card.step + Math.max(0, card.bench.state?.progress ?? 0)) / card.steps.length
        }
        QQC2.Label {
            text: card.complete
                ? qsTr("1000 is as fast as the reference machine (Ryzen 9 9950X3D, DDR5-6400, Samsung 980 PRO). Each part below counts as much as the others.")
                : qsTr("Run the CPU, memory and disk tests below to get the score of this machine.")
            opacity: 0.7
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        Repeater {
            model: card.parts
            DetailRow {
                required property var modelData
                label: modelData[0]
                value: modelData[1] > 0 ? qsTr("%1 points").arg(Format.num(modelData[1], 0)) : "-"
                Layout.fillWidth: true
            }
        }
    }
}
