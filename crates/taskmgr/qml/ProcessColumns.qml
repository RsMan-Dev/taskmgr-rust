import QtQuick
import org.kde.kirigami as Kirigami

// Metric columns of the Processes page: key, header, header total, value,
// cell text, and a width fitted to the widest value the column can show
// (not the current one), so widths stay put while numbers change.
Item {
    id: root

    property var snapshot: null

    // Header totals, the only part that changes with each sample.
    readonly property var totals: {
        const s = snapshot
        if (!s) {
            return ({})
        }
        const out = {
            cpu: Format.percent(s.cpu.usage * factor),
            memory: Format.percent(s.memory.usage),
            disk: Format.num(s.disks.reduce((a, d) => a + d.read_bps + d.write_bps, 0) / 1048576, 1) + " " + qsTr("MB/s"),
            net: Format.bits(s.networks.reduce((a, n) => a + n.rx_bps + n.tx_bps, 0)),
            threads: Format.num(s.cpu.threads, 0),
        }
        for (const g of s.gpus.filter(g => g.dedicated)) {
            out["gpu" + g.index] = Format.percent(g.usage)
            out["gpumem" + g.index] = g.memory_total ? Format.percent(g.memory_used / g.memory_total * 100) : ""
        }
        return out
    }

    // Rebuilt only when the processor count or the dedicated GPUs change,
    // so rows keep their cells between samples.
    readonly property string hardware: snapshot
        ? snapshot.cpu.logical + "/" + snapshot.gpus.filter(g => g.dedicated).map(g => g.index + ":" + g.name).join("|") : ""
    property int logical: 1
    property var dedicatedGpus: []
    readonly property int factor: Prefs.cpuPerCore ? logical : 1
    onHardwareChanged: {
        if (snapshot) {
            logical = snapshot.cpu.logical
            dedicatedGpus = snapshot.gpus.filter(g => g.dedicated).map(g => ({ index: g.index, name: g.name }))
        }
    }

    readonly property var allColumns: {
        const factor = root.factor
        const cols = [
            { key: "cpu", title: qsTr("CPU"),
              value: x => x.cpu * factor, reference: 25 * factor, text: v => Format.percent(v, 1),
              widest: [Format.percent(100 * factor, 1)] },
            { key: "memory", title: qsTr("Memory"),
              value: x => x.memory, reference: 2 * 1024 * 1024 * 1024, text: v => Format.bytes(v), widest: bytesWidest },
            { key: "disk", title: qsTr("Disk"),
              value: x => x.disk_bps ?? -1, reference: 20 * 1024 * 1024,
              text: v => v < 0 ? "" : Format.num(v / 1048576, 1) + " " + qsTr("MB/s"),
              widest: [Format.num(999.9, 1) + " " + qsTr("MB/s")] },
            { key: "net", title: qsTr("Network"),
              value: x => x.net ? x.net[0] + x.net[1] : -1, reference: 5 * 1024 * 1024,
              text: v => v < 0 ? "" : Format.bits(v), widest: [Format.bits(999e6 / 8), Format.bits(99.9e6 / 8)] },
            { key: "energy", title: qsTr("Energy"), tip: qsTr("Estimated from CPU and GPU use"),
              value: x => energyScore(x), reference: 15, text: v => energyLevel(v),
              widest: [qsTr("Very low"), qsTr("Moderate"), qsTr("Very high")] },
        ]
        for (const g of dedicatedGpus) {
            cols.push({ key: "gpu" + g.index, title: qsTr("GPU %1").arg(g.index), tip: g.name,
                        value: x => x.gpu?.[g.index] ?? 0, reference: 50, text: v => Format.percent(v), widest: [Format.percent(100)] })
            cols.push({ key: "gpumem" + g.index, title: qsTr("GPU %1 mem.").arg(g.index), tip: g.name,
                        value: x => x.gpu_memory?.[g.index] ?? 0, reference: 4 * 1024 * 1024 * 1024, text: v => v > 0 ? Format.bytes(v) : "",
                        widest: bytesWidest })
        }
        cols.push({ key: "threads", title: qsTr("Threads"),
                    value: x => x.threads ?? (x.processes ?? []).reduce((a, p) => a + p.threads, 0), reference: 1e9, text: v => Format.num(v, 0),
                    widest: [Format.num(99999, 0)] })
        for (const c of cols) {
            c.width = fit(c)
        }
        return cols
    }
    readonly property var columns: allColumns.filter(c => !Prefs.isHidden(c.key))
    readonly property var bytesWidest: [Format.bytes(999 * 1048576), Format.bytes(99.9 * 1073741824)]

    visible: false

    // Like Windows' "power usage": a level, not watts, since per-process
    // energy cannot be measured without root.
    function energyScore(x) {
        let gpu = 0
        for (const k in x.gpu ?? {}) {
            gpu += x.gpu[k]
        }
        return x.cpu + gpu * 0.5
    }

    function energyLevel(score) {
        if (score < 0.1) return qsTr("Very low")
        if (score < 1) return qsTr("Low")
        if (score < 5) return qsTr("Moderate")
        if (score < 15) return qsTr("High")
        return qsTr("Very high")
    }

    function fit(c) {
        const values = Math.max(...c.widest.map(t => Math.max(bigMetrics.advanceWidth(t), boldMetrics.advanceWidth(t))))
        const title = metrics.advanceWidth(c.title) + Kirigami.Units.iconSizes.small + Kirigami.Units.smallSpacing
        return Math.ceil(Math.max(values, title)) + Kirigami.Units.largeSpacing * 2
    }

    FontMetrics {
        id: metrics
        font: Kirigami.Theme.defaultFont
    }
    FontMetrics {
        id: boldMetrics
        font.family: Kirigami.Theme.defaultFont.family
        font.pointSize: Kirigami.Theme.defaultFont.pointSize
        font.weight: Font.DemiBold
    }
    FontMetrics {
        id: bigMetrics
        font.family: Kirigami.Theme.defaultFont.family
        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.35
    }
}
