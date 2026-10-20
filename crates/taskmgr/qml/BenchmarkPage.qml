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

    // Test method of the figures shown (bench::score::VERSION): older runs
    // only appear in the history.
    readonly property int method: 1

    // Newest run of a kind, and the one before it for the comparison.
    function runs(kind) {
        return history.filter(r => r.kind === kind && (r.version ?? 0) === method).slice(-2).reverse()
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
        const points = (r.version ?? 0) >= 1
        if (r.cpu) {
            return points ? qsTr("%1 points on one thread, %2 on all").arg(Format.num(r.cpu.single, 0)).arg(Format.num(r.cpu.multi, 0))
                : qsTr("%1 single, %2 multi (Mops/s)").arg(Format.num(r.cpu.single, 0)).arg(Format.num(r.cpu.multi, 0))
        }
        const score = points && r.points ? "  \u00b7  " + qsTr("%1 points").arg(Format.num(r.points, 0)) : ""
        if (r.memory) {
            return qsTr("read %1, write %2, copy %3 GB/s, %4 ns").arg(Format.num(r.memory.read, 1))
                .arg(Format.num(r.memory.write, 1)).arg(Format.num(r.memory.copy, 1)).arg(Format.num(r.memory.latency_ns, 1)) + score
        }
        if (r.disk) {
            return qsTr("write %1, read %2 MB/s, %3 IOPS (%4)").arg(Format.num(r.disk.write, 0))
                .arg(Format.num(r.disk.read, 0)).arg(Format.num(r.disk.random_iops, 0)).arg(r.disk.folder) + score
        }
        if (r.stability) {
            return qsTr("%1, %2 errors").arg(Format.duration(r.stability.seconds)).arg(r.stability.errors)
        }
        return ""
    }

    ColumnLayout {
        width: page.availableWidth
        spacing: page.unit

        PageHeader {
            title: qsTr("Benchmark")
        }

        MachineScore {
            bench: page
            onStartAll: page.monitor.startBenchmark("all", page.folder)
            cpu: page.runs("cpu")[0]
            memory: page.runs("memory")[0]
            disk: page.runs("disk")[0]
            Layout.fillWidth: true
        }

        QQC2.Label {
            text: qsTr("Close other programs for comparable scores. Results are compared with your previous run.")
            opacity: 0.7
            wrapMode: Text.Wrap
            Layout.fillWidth: true
            Layout.leftMargin: page.unit
            Layout.rightMargin: page.unit
        }

        BenchCard {
            bench: page
            id: cpuCard
            kind: "cpu"
            title: qsTr("CPU")
            description: qsTr("Four workloads (integers, floating point, sorting, compression) on one thread, then on all %1 threads, three times each (about 40 seconds).")
                .arg(page.snapshot?.cpu.logical ?? "")
            runs: page.runs("cpu")
            onStart: page.monitor.startBenchmark("cpu", "")
            BenchScore {
                label: qsTr("One thread")
                values: cpuCard.runs.map(r => r.cpu.single)
                text: v => qsTr("%1 points").arg(Format.num(v, 0))
            }
            BenchScore {
                label: qsTr("All threads")
                values: cpuCard.runs.map(r => r.cpu.multi)
                text: v => qsTr("%1 points").arg(Format.num(v, 0))
            }
        }

        BenchCard {
            bench: page
            id: memoryCard
            kind: "memory"
            title: qsTr("Memory")
            description: qsTr("Read, write and copy bandwidth on all threads, then latency, on a block eight times larger than the processor cache, three times each (about 20 seconds).")
            runs: page.runs("memory")
            onStart: page.monitor.startBenchmark("memory", "")
            BenchScore {
                label: qsTr("Score")
                values: memoryCard.runs.map(r => r.points)
                text: v => qsTr("%1 points").arg(Format.num(v, 0))
            }
            BenchScore {
                label: qsTr("Read")
                values: memoryCard.runs.map(r => r.memory.read)
                text: v => qsTr("%1 GB/s").arg(Format.num(v, 1))
            }
            BenchScore {
                label: qsTr("Write")
                values: memoryCard.runs.map(r => r.memory.write)
                text: v => qsTr("%1 GB/s").arg(Format.num(v, 1))
            }
            BenchScore {
                label: qsTr("Copy")
                values: memoryCard.runs.map(r => r.memory.copy)
                text: v => qsTr("%1 GB/s").arg(Format.num(v, 1))
            }
            BenchScore {
                label: qsTr("Latency")
                values: memoryCard.runs.map(r => r.memory.latency_ns)
                lowerIsBetter: true
                text: v => qsTr("%1 ns").arg(Format.num(v, 1))
            }
        }

        BenchCard {
            bench: page
            id: diskCard
            kind: "disk"
            title: qsTr("Disk")
            description: qsTr("Writes then reads a 1 GB file of random data in the folder below without the system cache, then random 4 KB reads on one and on eight threads (deleted afterwards).")
            runs: page.runs("disk")
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
            BenchScore {
                label: qsTr("Score")
                values: diskCard.runs.map(r => r.points)
                text: v => qsTr("%1 points").arg(Format.num(v, 0))
            }
            BenchScore {
                label: qsTr("Sequential write")
                values: diskCard.runs.map(r => r.disk.write)
                text: v => qsTr("%1 MB/s").arg(Format.num(v, 0))
            }
            BenchScore {
                label: qsTr("Sequential read")
                values: diskCard.runs.map(r => r.disk.read)
                text: v => qsTr("%1 MB/s").arg(Format.num(v, 0))
            }
            BenchScore {
                label: qsTr("Random 4 KB reads")
                values: diskCard.runs.map(r => r.disk.random_iops)
                text: v => qsTr("%1 IOPS").arg(Format.num(v, 0))
            }
            BenchScore {
                label: qsTr("Random 4 KB reads, 8 threads")
                values: diskCard.runs.map(r => r.disk.random_iops_parallel)
                text: v => qsTr("%1 IOPS").arg(Format.num(v, 0))
            }
            QQC2.Label {
                visible: diskCard.runs.length > 0 && !diskCard.runs[0].disk.direct
                text: qsTr("This filesystem does not allow reading past the system cache: the read figures may be too high.")
                color: Kirigami.Theme.neutralTextColor
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
        }

        BenchCard {
            bench: page
            id: stabilityCard
            kind: "stability"
            title: qsTr("Stability")
            description: qsTr("Loads every thread with calculations whose result is known in advance. An unstable overclock miscalculates before it crashes: any error means it is not stable.")
            runs: page.runs("stability")
            readonly property var last: runs.map(r => r.stability)
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
