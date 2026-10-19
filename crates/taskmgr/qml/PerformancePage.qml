import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: page

    required property var snapshot
    required property var monitor

    readonly property var colors: ({
        cpu: Kirigami.Theme.highlightColor,
        memory: "#9b59b6",
        disk: "#1d99f3",
        network: "#f67400",
        gpu: "#1abc9c",
        sensors: "#da4453",
        temperature: "#da4453",
        clock: "#f67400",
        voltage: "#fdbc4b",
        power: "#27ae60",
    })

    property string selectedKey: "cpu"
    property bool logicalProcessors: false

    // Changes only when a device comes or goes, so delegates and selection
    // survive updates.
    readonly property string keyList: {
        const s = snapshot
        if (!s) {
            return ""
        }
        return ["cpu", "memory"]
            .concat(s.disks.map(d => "disk:" + d.name))
            .concat(s.networks.map(n => "net:" + n.name))
            .concat(s.gpus.map(g => "gpu:" + g.pci_slot))
            .concat(s.sensors.length > 0 ? ["sensors"] : [])
            .join(",")
    }

    readonly property var selected: snapshot ? resource(selectedKey) ?? resource("cpu") : null

    function series(values, color) {
        return { values: values ?? [], color: color }
    }

    function details(rows) {
        return rows.filter(r => r[1] !== null && r[1] !== undefined && r[1] !== "")
    }

    // "usage % (speed | used / max) temperature", the sidebar summary.
    function summary(usage, middle, temperature) {
        return [usage, middle ? "(" + middle + ")" : "", temperature !== null && temperature !== undefined ? Format.temperature(temperature) : ""]
            .filter(x => x !== "").join("  ")
    }

    function mini(label, value, values, maximum, accent, unavailable) {
        return { label: label, value: value, values: values ?? [], maximum: maximum, accent: accent, unavailable: unavailable ?? "" }
    }

    function resource(key) {
        const s = snapshot
        const [kind, id] = [key.split(":")[0], key.slice(key.indexOf(":") + 1)]
        switch (kind) {
        case "cpu": {
            const c = s.cpu
            const ch = c.charts
            return {
                title: "Processeur",
                subtitle: "",
                summary: summary(Format.percent(c.usage), Format.ghz(c.mhz), c.temperature),
                header: c.model,
                color: colors.cpu,
                chartLabel: "% d'utilisation",
                maximum: 100,
                maximumLabel: "100 %",
                series: [series(c.history, colors.cpu)],
                cores: c.core_history,
                energy: true,
                charts: [
                    mini("Température", Format.temperature(c.temperature), ch.temperature, 100, colors.temperature,
                         c.temperature === null ? "Aucun capteur de température processeur" : ""),
                    mini("Fréquence moyenne", Format.mhz(c.mhz), ch.clock, c.max_mhz ?? 1, colors.clock),
                    mini("Tension", c.voltage !== null ? Format.num(c.voltage, 3) + " V" : "", ch.voltage, 1.5, colors.voltage,
                         c.voltage === null ? "Non exposée : aucun pilote de capteur de carte mère (nct6775, it87, zenpower)" : ""),
                ],
                stats: [
                    ["Utilisation", Format.percent(c.usage)],
                    ["Vitesse", Format.ghz(c.mhz)],
                    ["Processus", Format.num(c.processes, 0)],
                    ["Threads", Format.num(c.threads, 0)],
                    ["Temps de fonctionnement", Format.uptime(c.uptime_secs)],
                    ["Température", Format.temperature(c.temperature)],
                ].concat(s.energy.package_watts !== null ? [["Consommation", Format.watts(s.energy.package_watts)]] : []),
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
            const speed = m.dimms.length > 0 ? m.dimms[0].speed_mts : null
            return {
                title: "Mémoire",
                subtitle: m.dimms.length > 0 && m.dimms[0].kind ? m.dimms[0].kind + (speed ? " " + speed + " MT/s" : "") : "",
                summary: summary(Format.percent(m.usage), Format.bytes(m.used) + " / " + Format.bytes(m.total)),
                header: Format.bytes(m.total),
                color: colors.memory,
                chartLabel: "Utilisation de la mémoire",
                maximum: 100,
                maximumLabel: Format.bytes(m.total),
                series: [series(m.history, colors.memory)],
                dimms: m.dimms,
                stats: [
                    ["Utilisée", Format.bytes(m.used)],
                    ["Disponible", Format.bytes(m.available)],
                    ["Validée", Format.bytes(m.committed)],
                    ["En cache", Format.bytes(m.cached)],
                    ["Swap", m.swap_total ? Format.bytes(m.swap_used) + " / " + Format.bytes(m.swap_total) : "Aucun"],
                ],
                details: details([
                    ["Vitesse", speed ? speed + " MT/s" : null],
                    ["Type", m.dimms.length > 0 ? m.dimms[0].kind : null],
                    ["Emplacements utilisés", m.slots ? m.dimms.length + " sur " + m.slots : null],
                    ["Utilisable", Format.bytes(m.total)],
                ]),
            }
        }
        case "disk": {
            const d = s.disks.find(x => x.name === id)
            if (!d) {
                return null
            }
            const top = Format.niceMax(Math.max(...d.read_history, ...d.write_history, 0), 1024 * 1024)
            return {
                title: "Disque " + d.index + " (" + d.name + ")",
                subtitle: d.kind,
                summary: summary(Format.percent(d.active), Format.rate(d.read_bps + d.write_bps), d.temperature),
                header: d.model,
                color: colors.disk,
                chartLabel: "Temps d'activité (au moins une requête en cours)",
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
            const rxTop = Format.niceMax(Math.max(...n.rx_history, 0), 128 * 1024)
            const txTop = Format.niceMax(Math.max(...n.tx_history, 0), 128 * 1024)
            const load = n.link_mbps ? Math.max(n.rx_bps, n.tx_bps) * 8 / (n.link_mbps * 1e6) * 100 : null
            return {
                title: n.kind,
                subtitle: n.name,
                summary: summary(load !== null ? Format.percent(load) : "", "R " + Format.bits(n.rx_bps) + "  E " + Format.bits(n.tx_bps)),
                header: n.adapter,
                color: colors.network,
                chartLabel: "Réception",
                maximum: rxTop,
                maximumLabel: Format.bits(rxTop),
                series: [series(n.rx_history, colors.network)],
                secondChart: {
                    label: "Envoi",
                    maximum: txTop,
                    maximumLabel: Format.bits(txTop),
                    series: [series(n.tx_history, Qt.darker(colors.network, 1.4))],
                },
                stats: [
                    ["Réception", Format.bits(n.rx_bps)],
                    ["Envoi", Format.bits(n.tx_bps)],
                ].concat(load !== null ? [["Utilisation du lien", Format.percent(load)]] : []),
                details: details([
                    ["Adaptateur", n.adapter],
                    ["Interface", n.name],
                    ["Type de connexion", n.kind],
                    ["Vitesse de liaison", n.link_mbps ? Format.link(n.link_mbps) : null],
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
            const ch = g.charts
            const mem = g.memory_total ? Format.bytes(g.memory_used) + " / " + Format.bytes(g.memory_total) : null
            const noVoltage = g.driver === "nvidia" ? "Non exposée par le pilote NVIDIA" : "Non exposée par le pilote"
            return {
                title: "GPU " + g.index,
                subtitle: g.name,
                summary: summary(g.usage !== null ? Format.percent(g.usage) : "Non mesuré", mem, g.temperature),
                header: g.name,
                color: colors.gpu,
                chartLabel: "Utilisation",
                maximum: 100,
                maximumLabel: "100 %",
                series: [series(g.history, colors.gpu)],
                charts: [
                    mini(g.dedicated ? "Mémoire dédiée" : "Mémoire réservée", mem ?? "", ch.memory, 100, colors.memory,
                         g.memory_total ? "" : "Non mesurée"),
                    mini("Fréquence GPU", Format.mhz(g.core_mhz), ch.core_clock, g.core_max_mhz ?? 1, colors.clock,
                         g.core_mhz === null ? "Non mesurée" : ""),
                    mini("Fréquence mémoire", Format.mhz(g.memory_mhz), ch.memory_clock, g.memory_max_mhz ?? 1, colors.clock,
                         g.memory_mhz === null ? (g.dedicated ? "Non mesurée" : "Mémoire système partagée") : ""),
                    mini("Tension", Format.millivolts(g.voltage_mv), ch.voltage, 1500, colors.voltage,
                         g.voltage_mv === null ? noVoltage : ""),
                    mini("Consommation", Format.watts(g.power_watts) + (g.power_limit_watts ? " / " + Format.watts(g.power_limit_watts) : ""),
                         ch.power, g.power_limit_watts ?? 1, colors.power, g.power_watts === null ? "Non mesurée" : ""),
                ],
                stats: [
                    ["Utilisation", Format.percent(g.usage)],
                    ["Mémoire dédiée", mem ?? "—"],
                ].concat(g.shared_total ? [["Mémoire partagée", Format.bytes(g.shared_used) + " / " + Format.bytes(g.shared_total)]] : [])
                 .concat(g.temperature !== null ? [["Température", Format.temperature(g.temperature)]] : [])
                 .concat(g.encoder !== null ? [["Encodage vidéo", Format.percent(g.encoder)]] : [])
                 .concat(g.decoder !== null ? [["Décodage vidéo", Format.percent(g.decoder)]] : []),
                details: details([
                    ["Type", g.dedicated ? "Dédié" : "Intégré"],
                    ["Pilote", g.driver],
                    ["Version du pilote", g.driver_version],
                    ["Fréquence max", g.core_max_mhz ? Format.mhz(g.core_max_mhz) : null],
                    ["Emplacement PCI", g.pci_slot],
                ]),
            }
        }
        case "sensors": {
            const all = [].concat(...s.sensors.map(g => g.sensors))
            const hottest = all.reduce((a, b) => b.value > a.value ? b : a)
            return {
                title: "Capteurs",
                subtitle: all.length + " capteurs de température",
                summary: "Max " + Format.temperature(hottest.value),
                header: "",
                color: colors.sensors,
                maximum: 110,
                series: [series(hottest.history, colors.sensors)],
                sensors: true,
            }
        }
        }
        return null
    }

    function detailsText(r) {
        return [r.title + "  " + r.header]
            .concat((r.stats ?? []).map(x => x[0] + " : " + x[1]))
            .concat((r.details ?? []).map(x => x[0] + " : " + x[1]))
            .join("\n")
    }

    component Caption: QQC2.Label {
        opacity: 0.7
        font: Kirigami.Theme.smallFont
    }

    // Caption, chart and time axis. `chart` holds label, maximum,
    // maximumLabel and series.
    component ChartBlock: ColumnLayout {
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
                text: "60 secondes"
                Layout.fillWidth: true
            }
            Caption {
                text: "0"
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
                Layout.preferredWidth: Kirigami.Units.gridUnit * 16
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
                    // Selection follows the key: the view resets its index
                    // whenever a device comes or goes.
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
                    readonly property real inner: width - Kirigami.Units.gridUnit * 2
                    visible: r !== null
                    width: detailScroll.availableWidth
                    spacing: Kirigami.Units.largeSpacing

                    Item {
                        Layout.preferredHeight: Kirigami.Units.smallSpacing
                    }

                    ColumnLayout {
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true
                        Layout.leftMargin: Kirigami.Units.gridUnit
                        Layout.rightMargin: Kirigami.Units.gridUnit
                        Layout.bottomMargin: Kirigami.Units.gridUnit

                        RowLayout {
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

                        EnergyNotice {
                            visible: (!!detail.r?.energy || !!detail.r?.sensors) && page.snapshot?.energy?.status === "denied"
                            monitor: page.monitor
                            energy: page.snapshot?.energy ?? null
                            Layout.fillWidth: true
                        }

                        SensorsView {
                            visible: !!detail.r?.sensors
                            groups: detail.r?.sensors ? page.snapshot.sensors : []
                            Layout.fillWidth: true
                        }

                        ChartBlock {
                            id: mainChart
                            visible: !detail.r?.sensors && !(detail.r?.cores && page.logicalProcessors)
                            chart: detail.r
                            accent: detail.r?.color ?? "gray"
                            chartHeight: Math.max(Kirigami.Units.gridUnit * 7, page.height * (detail.r?.secondChart || detail.r?.charts ? 0.3 : 0.42))
                            Layout.fillWidth: true
                            onContextMenuRequested: (position) => chartMenu.popup()
                        }

                        // Windows-style "logical processors" view of the CPU.
                        ColumnLayout {
                            visible: !!detail.r?.cores && page.logicalProcessors
                            spacing: Kirigami.Units.smallSpacing / 2
                            Layout.fillWidth: true

                            RowLayout {
                                Caption {
                                    text: "% d'utilisation par processeur logique"
                                    Layout.fillWidth: true
                                }
                                Caption {
                                    text: "100 %"
                                }
                            }
                            GridLayout {
                                id: coreGrid
                                readonly property int count: detail.r?.cores?.length ?? 0
                                columns: Math.max(1, Math.ceil(Math.sqrt(count * 2)))
                                columnSpacing: Kirigami.Units.smallSpacing
                                rowSpacing: Kirigami.Units.smallSpacing
                                Layout.fillWidth: true

                                Repeater {
                                    model: coreGrid.count
                                    UsageChart {
                                        required property int index
                                        compact: true
                                        series: [{ values: detail.r?.cores?.[index] ?? [], color: page.colors.cpu }]
                                        accent: page.colors.cpu
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: Math.max(Kirigami.Units.gridUnit * 2.5,
                                            page.height * 0.42 / Math.ceil(coreGrid.count / coreGrid.columns))

                                        QQC2.ToolTip.text: "Processeur " + index
                                        QQC2.ToolTip.visible: coreHover.hovered
                                        HoverHandler {
                                            id: coreHover
                                        }
                                        TapHandler {
                                            acceptedButtons: Qt.RightButton
                                            onTapped: chartMenu.popup()
                                        }
                                    }
                                }
                            }
                        }

                        ChartBlock {
                            visible: !!detail.r?.secondChart
                            chart: detail.r?.secondChart ?? null
                            accent: detail.r?.color ?? "gray"
                            chartHeight: Math.max(Kirigami.Units.gridUnit * 5, page.height * 0.18)
                            Layout.fillWidth: true
                        }

                        GridLayout {
                            visible: (detail.r?.charts?.length ?? 0) > 0
                            columns: Math.max(1, Math.min(detail.r?.charts?.length ?? 1, Math.floor(detail.inner / (Kirigami.Units.gridUnit * 11))))
                            columnSpacing: Kirigami.Units.gridUnit
                            rowSpacing: Kirigami.Units.largeSpacing
                            Layout.fillWidth: true

                            Repeater {
                                model: detail.r?.charts ?? []
                                MiniChart {
                                    required property var modelData
                                    label: modelData.label
                                    value: modelData.value
                                    values: modelData.values
                                    maximum: modelData.maximum
                                    accent: modelData.accent
                                    unavailable: modelData.unavailable
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 1
                                }
                            }
                        }

                        // Live values and static details side by side, stacked when narrow.
                        GridLayout {
                            visible: !detail.r?.sensors
                            columns: detail.width > Kirigami.Units.gridUnit * 34 ? 2 : 1
                            columnSpacing: Kirigami.Units.gridUnit * 2
                            rowSpacing: Kirigami.Units.gridUnit
                            Layout.fillWidth: true

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

                        MemoryModules {
                            visible: (detail.r?.dimms?.length ?? 0) > 0
                            dimms: detail.r?.dimms ?? []
                            memory: page.snapshot?.memory ?? null
                            accent: page.colors.memory
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }

    QQC2.Menu {
        id: chartMenu

        QQC2.MenuItem {
            visible: !!page.selected?.cores
            height: visible ? implicitHeight : 0
            text: "Utilisation globale"
            checkable: true
            checked: !page.logicalProcessors
            onTriggered: page.logicalProcessors = false
        }
        QQC2.MenuItem {
            visible: !!page.selected?.cores
            height: visible ? implicitHeight : 0
            text: "Processeurs logiques"
            checkable: true
            checked: page.logicalProcessors
            onTriggered: page.logicalProcessors = true
        }
        QQC2.MenuSeparator {
            visible: !!page.selected?.cores
            height: visible ? implicitHeight : 0
        }
        QQC2.MenuItem {
            text: "Copier les détails"
            icon.name: "edit-copy"
            onTriggered: {
                clipboard.text = page.detailsText(page.selected)
                clipboard.selectAll()
                clipboard.copy()
            }
        }
    }

    TextEdit {
        id: clipboard
        visible: false
    }
}
