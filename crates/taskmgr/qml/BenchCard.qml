import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// One benchmark: title, run button, progress, error, and its figures.
Card {
    id: card

    required property var bench
    property string kind
    property string title
    property string description
    default property alias content: body.data
    // Newest runs first, measured the current way.
    property var runs: []
    readonly property bool mine: bench.running && bench.state?.kind === kind
    readonly property real busy: runs.length > 0 ? runs[0].background ?? 0 : 0

    signal start()

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
                text: card.title
                level: 3
                Layout.fillWidth: true
            }
            QQC2.Button {
                text: card.mine ? qsTr("Stop") : qsTr("Run")
                icon.source: Icons.of(card.mine ? "stop" : "start", Kirigami.Theme.textColor)
                enabled: card.mine || !card.bench.running
                onClicked: card.mine ? card.bench.monitor.stopBenchmark() : card.start()
            }
        }
        QQC2.Label {
            text: card.description
            opacity: 0.7
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        ProgressLine {
            visible: card.mine
            value: card.bench.state?.progress ?? 0
        }
        QQC2.Label {
            visible: !card.bench.running && card.bench.state?.kind === card.kind && (card.bench.state?.error ?? "") !== ""
            text: Tr.core(card.bench.state?.error ?? "")
            color: Kirigami.Theme.negativeTextColor
            Layout.fillWidth: true
        }
        QQC2.Label {
            visible: card.busy > 0.05 && !card.mine
            text: qsTr("Other programs used %1 % of the processor during the last run: its figures may be low.")
                .arg(Format.num(card.busy * 100, 0))
            color: Kirigami.Theme.neutralTextColor
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        ColumnLayout {
            id: body
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
        }
    }
}
