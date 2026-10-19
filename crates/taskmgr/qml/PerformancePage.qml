import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Resource list on the left, details of the selected resource on the right.
// Every disk, adapter and GPU is its own entry.
Item {
    id: page

    required property var snapshot

    // Series colours: the accent colour for the CPU, Breeze hues otherwise.
    readonly property var colors: ({
        cpu: Kirigami.Theme.highlightColor,
        memory: "#9b59b6",
        disk: "#1d99f3",
        network: "#f67400",
        gpu: "#1abc9c",
    })

    property string selectedKey: "cpu"

    // Only changes when a device appears or disappears, so the list keeps
    // its delegates (and selection) across updates.
    readonly property string keyList: {
        const s = snapshot
        if (!s) {
            return ""
        }
        return ["cpu", "memory"]
            .concat(s.disks.map(d => "disk:" + d.name))
            .concat(s.networks.map(n => "net:" + n.name))
            .concat(s.gpus.map(g => "gpu:" + g.pci_slot))
            .join(",")
    }

    readonly property var selected: snapshot ? resource(selectedKey) ?? resource("cpu") : null

    function series(values, color) {
        return { values: values, color: color }
    }

    function details(rows) {
        return rows.filter(r => r[1] !== null && r[1] !== undefined && r[1] !== "")
    }

    function resource(key) {
        const s = snapshot
        const [kind, id] = [key.split(":")[0], key.slice(key.indexOf(":") + 1)]
        switch (kind) {
        case "cpu": {
            const c = s.cpu
            return {
                title: "Processeur",
                subtitle: "",
                summary: Format.percent(c.usage) + "  " + Format.ghz(c.mhz),
                header: c.model,
                color: colors.cpu,
                chartLabel: "% d'utilisation",
                maximum: 100,
                maximumLabel: "100 %",
                series: [series(c.history, colors.cpu)],
                stats: [
                    ["Utilisation", Format.percent(c.usage)],
                    ["Vitesse", Format.ghz(c.mhz)],
                    ["Processus", Format.num(c.processes, 0)],
                    ["Threads", Format.num(c.threads, 0)],
                    ["Temps de fonctionnement", Format.uptime(c.uptime_secs)],
                    ["Température", Format.temperature(c.temperature)],
                ],
                details: details([
                    ["Vitesse max", c.max_mhz ? Format.ghz(c.max_mhz) : null],
                    ["Sockets", c.sockets],
                    ["Cœurs", c.cores],
                    ["Processeurs logiques", c.logical],
                    ["Virtualisation", c.virtualization],
                ].concat(c.caches.map(k => ["Cache " + k.name, Format.bytes(k.bytes)]))),
            }
        }
        case "memory": {
            const m = s.memory
            return {
                title: "Mémoire",
                subtitle: "",
                summary: Format.bytes(m.used) + " / " + Format.bytes(m.total) + " (" + Format.percent(m.usage) + ")",
                header: Format.bytes(m.total),
                color: colors.memory,
                chartLabel: "Utilisation de la mémoire",
                maximum: 100,
                maximumLabel: Format.bytes(m.total),
                series: [series(m.history, colors.memory)],
                stats: [
                    ["Utilisée", Format.bytes(m.used)],
                    ["Disponible", Format.bytes(m.available)],
                    ["Validée", Format.bytes(m.committed)],
                    ["En cache", Format.bytes(m.cached)],
                    ["Swap", m.swap_total ? Format.bytes(m.swap_used) + " / " + Format.bytes(m.swap_total) : "Aucun"],
                ],
                details: [["Utilisable", Format.bytes(m.total)]],
            }
        }
        case "disk": {
            const d = s.disks.find(x => x.name === id)
            if (!d) {
                return null
            }
            const top = Format.niceMax(Math.max(...d.read_history, ...d.write_history), 1024 * 1024)
            return {
                title: "Disque " + d.index + " (" + d.name + ")",
                subtitle: d.kind,
                summary: Format.percent(d.active) + (d.temperature !== null ? "  " + Format.temperature(d.temperature) : ""),
                header: d.model,
                color: colors.disk,
                chartLabel: "Temps d'activité",
                maximum: 100,
                maximumLabel: "100 %",
                series: [series(d.active_history, colors.disk)],
                secondChart: {
                    label: "Vitesse de transfert (lecture, écriture)",
                    maximum: top,
                    maximumLabel: Format.rate(top),
                    series: [series(d.read_history, colors.disk), series(d.write_history, Qt.darker(colors.disk, 1.6))],
                },
                stats: [
                    ["Temps d'activité", Format.percent(d.active)],
                    ["Temps de réponse moyen", Format.num(d.response_ms, 1) + " ms"],
                    ["Vitesse de lecture", Format.rate(d.read_bps)],
                    ["Vitesse d'écriture", Format.rate(d.write_bps)],
                ].concat(d.temperature !== null ? [["Température", Format.temperature(d.temperature)]] : []),
                details: details([
                    ["Capacité", Format.bytes(d.capacity)],
                    ["Type", d.kind],
                    ["Disque système", d.system_disk ? "Oui" : "Non"],
                    ["Points de montage", d.mounts.join(", ") || "Aucun"],
                ]),
            }
        }
        case "net": {
            const n = s.networks.find(x => x.name === id)
            if (!n) {
                return null
            }
            const top = Format.niceMax(Math.max(...n.rx_history, ...n.tx_history), 128 * 1024)
            return {
                title: n.kind,
                subtitle: n.name,
                summary: "E : " + Format.bits(n.tx_bps) + "  R : " + Format.bits(n.rx_bps),
                header: n.adapter,
                color: colors.network,
                chartLabel: "Débit (réception, envoi)",
                maximum: top,
                maximumLabel: Format.bits(top),
                series: [series(n.rx_history, colors.network), series(n.tx_history, Qt.darker(colors.network, 1.6))],
                stats: [
                    ["Envoi", Format.bits(n.tx_bps)],
                    ["Réception", Format.bits(n.rx_bps)],
                ],
                details: details([
                    ["Adaptateur", n.adapter],
                    ["Interface", n.name],
                    ["Type de connexion", n.kind],
                    ["Vitesse de liaison", n.link_mbps ? (n.link_mbps >= 1000 ? Format.num(n.link_mbps / 1000, 1) + " Gb/s" : n.link_mbps + " Mb/s") : null],
                    ["Adresse IPv4", n.ipv4.join("\n")],
                    ["Adresse IPv6", n.ipv6.join("\n")],
                    ["Adresse MAC", n.mac],
                ]),
            }
        }
        case "gpu": {
            const g = s.gpus.find(x => x.pci_slot === id)
            if (!g) {
                return null
            }
            const mem = g.memory_total ? Format.bytes(g.memory_used) + " / " + Format.bytes(g.memory_total) : null
            const temp = g.temperature !== null ? " (" + Format.temperature(g.temperature) + ")" : ""
            return {
                title: "GPU " + g.index,
                subtitle: g.name,
                summary: (g.usage !== null ? Format.percent(g.usage) : "Non mesuré") + temp,
                header: g.name,
                color: colors.gpu,
                chartLabel: "Utilisation",
                maximum: 100,
                maximumLabel: "100 %",
                series: [series(g.history, colors.gpu)],
                secondChart: g.memory_total ? {
                    label: "Utilisation de la mémoire dédiée",
                    maximum: 100,
                    maximumLabel: Format.bytes(g.memory_total),
                    series: [series(g.memory_history, colors.gpu)],
                } : null,
                stats: [
                    ["Utilisation", Format.percent(g.usage)],
                    ["Mémoire dédiée", mem ?? "—"],
                ].concat(g.shared_total ? [["Mémoire partagée", Format.bytes(g.shared_used) + " / " + Format.bytes(g.shared_total)]] : [])
                 .concat(g.temperature !== null ? [["Température", Format.temperature(g.temperature)]] : [])
                 .concat(g.power_watts !== null ? [["Consommation", Format.num(g.power_watts, 0) + " W"]] : [])
                 .concat(g.encoder !== null ? [["Encodage vidéo", Format.percent(g.encoder)]] : [])
                 .concat(g.decoder !== null ? [["Décodage vidéo", Format.percent(g.decoder)]] : []),
                details: details([
                    ["Pilote", g.driver],
                    ["Version du pilote", g.driver_version],
                    ["Emplacement PCI", g.pci_slot],
                ]),
            }
        }
        }
        return null
    }

    // Caption, chart and time axis. `chart` holds label, maximum,
    // maximumLabel and series.
    component ChartBlock: ColumnLayout {
        id: block
        property var chart
        property color accent
        property real chartHeight

        spacing: Kirigami.Units.smallSpacing / 2
        Layout.leftMargin: Kirigami.Units.gridUnit
        Layout.rightMargin: Kirigami.Units.gridUnit

        RowLayout {
            QQC2.Label {
                text: block.chart?.chartLabel ?? block.chart?.label ?? ""
                opacity: 0.7
                font: Kirigami.Theme.smallFont
            }
            Item { Layout.fillWidth: true }
            QQC2.Label {
                text: block.chart?.maximumLabel ?? ""
                opacity: 0.7
                font: Kirigami.Theme.smallFont
            }
        }
        UsageChart {
            series: block.chart?.series ?? []
            maximum: block.chart?.maximum ?? 100
            accent: block.accent
            Layout.fillWidth: true
            Layout.preferredHeight: block.chartHeight
        }
        RowLayout {
            QQC2.Label {
                text: "60 secondes"
                opacity: 0.7
                font: Kirigami.Theme.smallFont
            }
            Item { Layout.fillWidth: true }
            QQC2.Label {
                text: "0"
                opacity: 0.7
                font: Kirigami.Theme.smallFont
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        PageHeader {
            title: "Performances"
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            QQC2.ScrollView {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 15
                Layout.fillWidth: false
                Layout.fillHeight: true
                QQC2.ScrollBar.horizontal.policy: QQC2.ScrollBar.AlwaysOff

                ListView {
                    id: list
                    model: page.keyList === "" ? [] : page.keyList.split(",")
                    spacing: Kirigami.Units.smallSpacing
                    topMargin: Kirigami.Units.largeSpacing
                    leftMargin: Kirigami.Units.largeSpacing
                    rightMargin: Kirigami.Units.largeSpacing
                    bottomMargin: Kirigami.Units.largeSpacing
                    clip: true
                    // Selection follows the key, not ListView.currentIndex: the
                    // view resets its index whenever a device comes or goes.
                    currentIndex: -1
                    activeFocusOnTab: true

                    function step(delta) {
                        const keys = model
                        const i = Math.max(0, keys.indexOf(page.selectedKey)) + delta
                        if (i >= 0 && i < keys.length) {
                            page.selectedKey = keys[i]
                            positionViewAtIndex(i, ListView.Contain)
                        }
                    }
                    Keys.onUpPressed: step(-1)
                    Keys.onDownPressed: step(1)

                    delegate: ResourceCard {
                        required property string modelData
                        required property int index
                        width: ListView.view.width - ListView.view.leftMargin - ListView.view.rightMargin
                        resource: page.resource(modelData) ?? { title: "", subtitle: "", summary: "", series: [], maximum: 100, color: "gray" }
                        selected: modelData === page.selectedKey
                        onClicked: {
                            page.selectedKey = modelData
                            ListView.view.forceActiveFocus()
                        }
                    }
                }
            }

            Kirigami.Separator {
                Layout.fillHeight: true
            }

            QQC2.ScrollView {
                id: detailScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: availableWidth

                ColumnLayout {
                    id: detail
                    readonly property var r: page.selected
                    visible: r !== null
                    width: detailScroll.availableWidth
                    spacing: Kirigami.Units.largeSpacing

                    // Outer margins: a ColumnLayout has none of its own.
                    Item { Layout.preferredHeight: Kirigami.Units.smallSpacing }

                    RowLayout {
                        Layout.leftMargin: Kirigami.Units.gridUnit
                        Layout.rightMargin: Kirigami.Units.gridUnit
                        Kirigami.Heading {
                            text: detail.r?.title ?? ""
                            level: 1
                        }
                        QQC2.Label {
                            text: detail.r?.header ?? ""
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.2
                            horizontalAlignment: Text.AlignRight
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }

                    ChartBlock {
                        chart: detail.r
                        accent: detail.r?.color ?? "gray"
                        chartHeight: Math.max(Kirigami.Units.gridUnit * 8, page.height * (detail.r?.secondChart ? 0.32 : 0.45))
                    }

                    ChartBlock {
                        visible: !!detail.r?.secondChart
                        chart: detail.r?.secondChart ?? null
                        accent: detail.r?.color ?? "gray"
                        chartHeight: Math.max(Kirigami.Units.gridUnit * 5, page.height * 0.18)
                    }

                    // Live values and static details side by side, stacked when narrow.
                    GridLayout {
                        columns: detail.width > Kirigami.Units.gridUnit * 34 ? 2 : 1
                        columnSpacing: Kirigami.Units.gridUnit * 2
                        rowSpacing: Kirigami.Units.gridUnit
                        Layout.leftMargin: Kirigami.Units.gridUnit
                        Layout.rightMargin: Kirigami.Units.gridUnit
                        Layout.bottomMargin: Kirigami.Units.gridUnit

                        GridLayout {
                            columns: 2
                            columnSpacing: Kirigami.Units.gridUnit * 2
                            rowSpacing: Kirigami.Units.largeSpacing
                            Layout.alignment: Qt.AlignTop

                            Repeater {
                                model: detail.r?.stats ?? []
                                Stat {
                                    required property var modelData
                                    label: modelData[0]
                                    value: modelData[1]
                                }
                            }
                        }

                        ColumnLayout {
                            spacing: Kirigami.Units.smallSpacing
                            Layout.alignment: Qt.AlignTop
                            Layout.fillWidth: true

                            Repeater {
                                model: detail.r?.details ?? []
                                DetailRow {
                                    required property var modelData
                                    label: modelData[0]
                                    value: String(modelData[1])
                                    Layout.fillWidth: true
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
