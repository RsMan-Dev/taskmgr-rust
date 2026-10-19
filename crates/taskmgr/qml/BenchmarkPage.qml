import QtCore
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// CPU, memory and disk scores, and a stability test for overclocking.
ThinScrollView {
    id: page

    required property var monitor
    property var snapshot: null
    property bool active: false

    readonly property real unit: Kirigami.Units.gridUnit
    readonly property var state: monitor.benchmark !== "" ? JSON.parse(monitor.benchmark) : null
    readonly property bool running: state?.running ?? false
    readonly property var history: monitor.benchmarkHistory !== "" ? JSON.parse(monitor.benchmarkHistory) : []
    property string folder: StandardPaths.writableLocation(StandardPaths.HomeLocation).toString().replace("file://", "")
    property int minutes: 15

    contentWidth: availableWidth

    onActiveChanged: if (active && monitor.benchmarkHistory === "") monitor.refreshBenchmarkHistory()

    // Newest run of a kind, and the one before it for the comparison.
    function runs(kind) {
        return history.filter(r => r.kind === kind).slice(-2).reverse()
    }

    function change(now, before, higherIsBetter) {
        if (!before || !now) {
            return ""
        }
        const pct = (now - before) / before * 100 * (higherIsBetter ? 1 : -1)
        return (pct >= 0 ? "+" : "") + Format.num(pct, 1) + " %"
    }

    function when(secs) {
        return Qt.formatDateTime(new Date(secs * 1000), "dd/MM/yyyy HH:mm")
    }

    function kindName(kind) {
        return {
            cpu: qsTr("CPU"),
            memory: qsTr("Memory"),
            disk: qsTr("Disk"),
            stability: qsTr("Stability"),
        }[kind] ?? kind
    }

    function summary(r) {
        if (r.cpu) {
            return qsTr("%1 single, %2 multi (Mops/s)").arg(Format.num(r.cpu.single, 0)).arg(Format.num(r.cpu.multi, 0))
        }
        if (r.memory) {
            return qsTr("read %1, write %2, copy %3 GB/s, %4 ns").arg(Format.num(r.memory.read, 1))
                .arg(Format.num(r.memory.write, 1)).arg(Format.num(r.memory.copy, 1)).arg(Format.num(r.memory.latency_ns, 1))
        }
        if (r.disk) {
            return qsTr("write %1, read %2 MB/s, %3 IOPS (%4)").arg(Format.num(r.disk.write, 0))
                .arg(Format.num(r.disk.read, 0)).arg(Format.num(r.disk.random_iops, 0)).arg(r.disk.folder)
        }
        if (r.stability) {
            return qsTr("%1, %2 errors").arg(Format.duration(r.stability.seconds)).arg(r.stability.errors)
        }
        return ""
    }

    // Newest value of a score, with the change since the previous run.
    component Score: DetailRow {
        property var values: []
        property var text: v => String(v)
        property bool lowerIsBetter: false
        visible: values.length > 0
        value: values.length > 0 ? text(values[0]) + "   " + page.change(values[0], values[1], !lowerIsBetter) : ""
        Layout.fillWidth: true
    }

    component TestCard: Card {
        id: card
        property string kind
        property string title
        property string description
        default property alias content: body.data
        property var last: []
        readonly property bool mine: page.running && page.state?.kind === kind
        Layout.leftMargin: page.unit
        Layout.rightMargin: page.unit
        implicitHeight: column.implicitHeight + page.unit * 1.5

        signal start()

        ColumnLayout {
            id: column
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: page.unit
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
                    enabled: card.mine || !page.running
                    onClicked: card.mine ? page.monitor.stopBenchmark() : card.start()
                }
            }
            QQC2.Label {
                text: card.description
                opacity: 0.7
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            QQC2.ProgressBar {
                visible: card.mine
                indeterminate: (page.state?.progress ?? 0) < 0
                value: Math.max(0, page.state?.progress ?? 0)
                Layout.fillWidth: true
            }
            QQC2.Label {
                visible: !page.running && page.state?.kind === card.kind && (page.state?.error ?? "") !== ""
                text: Tr.core(page.state?.error ?? "")
                color: Kirigami.Theme.negativeTextColor
                Layout.fillWidth: true
            }
            ColumnLayout {
                id: body
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true
            }
        }
    }

    ColumnLayout {
        width: page.availableWidth
        spacing: page.unit

        PageHeader {
            title: qsTr("Benchmark")
        }

        QQC2.Label {
            text: qsTr("Close other programs for comparable scores. Results are compared with your previous run.")
            opacity: 0.7
            wrapMode: Text.Wrap
            Layout.fillWidth: true
            Layout.leftMargin: page.unit
            Layout.rightMargin: page.unit
        }

        TestCard {
            id: cpuCard
            kind: "cpu"
            title: qsTr("CPU")
            description: qsTr("Integer and floating point work on one thread, then on all %1 threads (10 seconds).")
                .arg(page.snapshot?.cpu.logical ?? "")
            last: page.runs("cpu").map(r => r.cpu)
            onStart: page.monitor.startBenchmark("cpu", "")
            Score {
                label: qsTr("One thread")
                values: cpuCard.last.map(c => c.single)
                text: v => Format.num(v, 0) + " Mops/s"
            }
            Score {
                label: qsTr("All threads")
                values: cpuCard.last.map(c => c.multi)
                text: v => Format.num(v, 0) + " Mops/s"
            }
            Score {
                label: qsTr("Scaling")
                values: cpuCard.last.map(c => c.multi / Math.max(c.single, 1))
                text: v => qsTr("%1 x").arg(Format.num(v, 1))
            }
        }

        TestCard {
            id: memoryCard
            kind: "memory"
            title: qsTr("Memory")
            description: qsTr("Read, write and copy bandwidth on all threads, then latency (about 10 seconds).")
            last: page.runs("memory").map(r => r.memory)
            onStart: page.monitor.startBenchmark("memory", "")
            Score {
                label: qsTr("Read")
                values: memoryCard.last.map(m => m.read)
                text: v => qsTr("%1 GB/s").arg(Format.num(v, 1))
            }
            Score {
                label: qsTr("Write")
                values: memoryCard.last.map(m => m.write)
                text: v => qsTr("%1 GB/s").arg(Format.num(v, 1))
            }
            Score {
                label: qsTr("Copy")
                values: memoryCard.last.map(m => m.copy)
                text: v => qsTr("%1 GB/s").arg(Format.num(v, 1))
            }
            Score {
                label: qsTr("Latency")
                values: memoryCard.last.map(m => m.latency_ns)
                lowerIsBetter: true
                text: v => qsTr("%1 ns").arg(Format.num(v, 1))
            }
        }

        TestCard {
            id: diskCard
            kind: "disk"
            title: qsTr("Disk")
            description: qsTr("Writes then reads a 1 GB file in the folder below, then random 4 KB reads (deleted afterwards).")
            last: page.runs("disk").map(r => r.disk)
            onStart: page.monitor.startBenchmark("disk", page.folder)
            RowLayout {
                QQC2.Label {
                    text: qsTr("Folder")
                    opacity: 0.7
                }
                QQC2.TextField {
                    text: page.folder
                    onEditingFinished: page.folder = text
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    text: qsTr("Choose…")
                    icon.source: Icons.of("open-folder", Kirigami.Theme.textColor)
                    onClicked: folderDialog.open()
                }
            }
            Score {
                label: qsTr("Sequential write")
                values: diskCard.last.map(d => d.write)
                text: v => qsTr("%1 MB/s").arg(Format.num(v, 0))
            }
            Score {
                label: qsTr("Sequential read")
                values: diskCard.last.map(d => d.read)
                text: v => qsTr("%1 MB/s").arg(Format.num(v, 0))
            }
            Score {
                label: qsTr("Random 4 KB reads")
                values: diskCard.last.map(d => d.random_iops)
                text: v => qsTr("%1 IOPS").arg(Format.num(v, 0))
            }
        }

        TestCard {
            id: stabilityCard
            kind: "stability"
            title: qsTr("Stability")
            description: qsTr("Loads every thread with calculations whose result is known in advance. An unstable overclock miscalculates before it crashes: any error means it is not stable.")
            last: page.runs("stability").map(r => r.stability)
            onStart: page.monitor.startBenchmark("stability", String(page.minutes))
            readonly property bool live: mine && page.state?.blocks !== undefined
            readonly property var cpu: page.snapshot?.cpu ?? null
            RowLayout {
                QQC2.Label {
                    text: qsTr("Duration")
                    opacity: 0.7
                }
                QQC2.ComboBox {
                    readonly property var choices: [5, 15, 30, 60, 0]
                    enabled: !page.running
                    model: choices.map(m => m === 0 ? qsTr("Until stopped") : qsTr("%1 min").arg(m))
                    currentIndex: choices.indexOf(page.minutes)
                    implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                    onActivated: (i) => page.minutes = choices[i]
                }
            }
            DetailRow {
                visible: stabilityCard.live
                label: qsTr("Running for")
                value: Format.duration(page.state?.seconds ?? 0)
                Layout.fillWidth: true
            }
            DetailRow {
                visible: stabilityCard.live
                label: qsTr("Errors")
                value: String(page.state?.errors ?? 0)
                Layout.fillWidth: true
            }
            DetailRow {
                visible: stabilityCard.live && stabilityCard.cpu !== null
                label: qsTr("CPU now")
                value: stabilityCard.cpu ? [Format.ghz(stabilityCard.cpu.mhz), Format.temperature(stabilityCard.cpu.temperature),
                    page.snapshot.energy.package_watts !== null ? Format.watts(page.snapshot.energy.package_watts) : ""]
                    .filter(x => x).join("  \u00b7  ") : ""
                Layout.fillWidth: true
            }
            DetailRow {
                visible: !stabilityCard.live && stabilityCard.last.length > 0
                label: qsTr("Last run")
                value: stabilityCard.last.length > 0 ? (stabilityCard.last[0].errors === 0
                    ? qsTr("Stable for %1").arg(Format.duration(stabilityCard.last[0].seconds))
                    : qsTr("%1 errors in %2: not stable").arg(stabilityCard.last[0].errors).arg(Format.duration(stabilityCard.last[0].seconds))) : ""
                Layout.fillWidth: true
            }
        }

        FolderDialog {
            id: folderDialog
            currentFolder: "file://" + page.folder
            onAccepted: page.folder = decodeURIComponent(selectedFolder.toString().replace("file://", ""))
        }

        ColumnLayout {
            visible: page.history.length > 0
            spacing: Kirigami.Units.smallSpacing
            Layout.leftMargin: page.unit
            Layout.rightMargin: page.unit
            RowLayout {
                Kirigami.Heading {
                    text: qsTr("History")
                    level: 3
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    text: qsTr("Clear")
                    enabled: !page.running
                    onClicked: page.monitor.clearBenchmarkHistory()
                }
            }
            Repeater {
                model: page.history.slice().reverse()
                DetailRow {
                    required property var modelData
                    label: page.when(modelData.time) + "  " + page.kindName(modelData.kind)
                    value: page.summary(modelData)
                    labelWidth: page.unit * 11
                    Layout.fillWidth: true
                }
            }
        }

        Item {
            Layout.preferredHeight: page.unit
        }
    }
}
