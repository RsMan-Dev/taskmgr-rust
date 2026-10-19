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
                title: qsTr("CPU"),
                subtitle: "",
                summary: summary(Format.percent(c.usage), Format.ghz(c.mhz), c.temperature),
                header: c.model,
                color: colors.cpu,
                chartLabel: qsTr("% utilization"),
                maximum: 100,
                maximumLabel: "100 %",
                series: [series(c.history, colors.cpu)],
                cores: c.core_history,
                energy: true,
                charts: [
                    mini(qsTr("Temperature"), Format.temperature(c.temperature), ch.temperature, 100, colors.temperature,
                         c.temperature === null ? qsTr("No CPU temperature sensor") : ""),
                    mini(qsTr("Average clock"), Format.mhz(c.mhz), ch.clock, c.max_mhz ?? 1, colors.clock),
                    mini(qsTr("Voltage"), c.voltage !== null ? Format.num(c.voltage, 3) + " V" : "", ch.voltage, 1.5, colors.voltage,
                         c.voltage === null ? qsTr("Not exposed: no motherboard sensor driver (nct6775, it87, zenpower)") : ""),
                ],
                stats: [
                    [qsTr("Utilization"), Format.percent(c.usage)],
                    [qsTr("Speed"), Format.ghz(c.mhz)],
                    [qsTr("Processes"), Format.num(c.processes, 0)],
                    [qsTr("Threads"), Format.num(c.threads, 0)],
                    [qsTr("Up time"), Format.uptime(c.uptime_secs)],
                    [qsTr("Temperature"), Format.temperature(c.temperature)],
                ].concat(s.energy.package_watts !== null ? [[qsTr("Power"), Format.watts(s.energy.package_watts)]] : []),
                details: details([
                    [qsTr("Max speed"), c.max_mhz ? Format.ghz(c.max_mhz) : null],
                    [qsTr("Sockets"), c.sockets],
                    [qsTr("Cores"), c.cores],
                    [qsTr("Logical processors"), c.logical],
                    [qsTr("Virtualization"), c.virtualization],
                ].concat(c.caches.map(k => [qsTr("Cache %1").arg(k.name), Format.bytes(k.bytes)]))),
            }
        }
        case "memory": {
            const m = s.memory
            const speed = m.dimms.length > 0 ? m.dimms[0].speed_mts : null
            return {
                title: qsTr("Memory"),
                subtitle: m.dimms.length > 0 && m.dimms[0].kind ? m.dimms[0].kind + (speed ? " " + qsTr("%1 MT/s").arg(speed) : "") : "",
                summary: summary(Format.percent(m.usage), Format.bytes(m.used) + " / " + Format.bytes(m.total)),
                header: Format.bytes(m.total),
                color: colors.memory,
                chartLabel: qsTr("Memory usage"),
                maximum: 100,
                maximumLabel: Format.bytes(m.total),
                series: [series(m.history, colors.memory)],
                dimms: m.dimms,
                stats: [
                    [qsTr("In use"), Format.bytes(m.used)],
                    [qsTr("Available"), Format.bytes(m.available)],
                    [qsTr("Committed"), Format.bytes(m.committed)],
                    [qsTr("Cached"), Format.bytes(m.cached)],
                    [qsTr("Swap"), m.swap_total ? Format.bytes(m.swap_used) + " / " + Format.bytes(m.swap_total) : qsTr("None")],
                ],
                details: details([
                    [qsTr("Speed"), speed ? qsTr("%1 MT/s").arg(speed) : null],
                    [qsTr("Type"), m.dimms.length > 0 ? m.dimms[0].kind : null],
                    [qsTr("Slots used"), m.slots ? qsTr("%1 of %2").arg(m.dimms.length).arg(m.slots) : null],
                    [qsTr("Usable"), Format.bytes(m.total)],
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
                title: qsTr("Disk %1 (%2)").arg(d.index).arg(d.name),
                subtitle: Tr.core(d.kind),
                summary: summary(Format.percent(d.active), Format.rate(d.read_bps + d.write_bps), d.temperature),
                header: d.model,
                color: colors.disk,
                chartLabel: qsTr("Active time (at least one request in flight)"),
                maximum: 100,
                maximumLabel: "100 %",
                series: [series(d.active_history, colors.disk)],
                secondChart: {
                    label: qsTr("Transfer rate (read, write)"),
                    maximum: top,
                    maximumLabel: Format.rate(top),
                    series: [series(d.read_history, colors.disk), series(d.write_history, Qt.darker(colors.disk, 1.6))],
                },
                stats: [
                    [qsTr("Active time"), Format.percent(d.active)],
                    [qsTr("Average response time"), qsTr("%1 ms").arg(Format.num(d.response_ms, 1))],
                    [qsTr("Read speed"), Format.rate(d.read_bps)],
                    [qsTr("Write speed"), Format.rate(d.write_bps)],
                ].concat(d.temperature !== null ? [[qsTr("Temperature"), Format.temperature(d.temperature)]] : []),
                details: details([
                    [qsTr("Capacity"), Format.bytes(d.capacity)],
                    [qsTr("Type"), Tr.core(d.kind)],
                    [qsTr("System disk"), d.system_disk ? qsTr("Yes") : qsTr("No")],
                    [qsTr("Mount points"), d.mounts.join(", ") || qsTr("None")],
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
                summary: summary(load !== null ? Format.percent(load) : "", qsTr("R %1  S %2").arg(Format.bits(n.rx_bps)).arg(Format.bits(n.tx_bps))),
                header: n.adapter,
                color: colors.network,
                chartLabel: qsTr("Receive"),
                maximum: rxTop,
                maximumLabel: Format.bits(rxTop),
                series: [series(n.rx_history, colors.network)],
                secondChart: {
                    label: qsTr("Send"),
                    maximum: txTop,
                    maximumLabel: Format.bits(txTop),
                    series: [series(n.tx_history, Qt.darker(colors.network, 1.4))],
                },
                stats: [
                    [qsTr("Receive"), Format.bits(n.rx_bps)],
                    [qsTr("Send"), Format.bits(n.tx_bps)],
                ].concat(load !== null ? [[qsTr("Link utilization"), Format.percent(load)]] : []),
                details: details([
                    [qsTr("Adapter"), n.adapter],
                    [qsTr("Interface"), n.name],
                    [qsTr("Connection type"), n.kind],
                    [qsTr("Link speed"), n.link_mbps ? Format.link(n.link_mbps) : null],
                    [qsTr("IPv4 address"), n.ipv4.join("\n")],
                    [qsTr("IPv6 address"), n.ipv6.join("\n")],
                    [qsTr("MAC address"), n.mac],
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
            const noVoltage = g.driver === "nvidia" ? qsTr("Not exposed by the NVIDIA driver") : qsTr("Not exposed by the driver")
            return {
                title: qsTr("GPU %1").arg(g.index),
                subtitle: g.name,
                summary: summary(g.usage !== null ? Format.percent(g.usage) : qsTr("Not measured"), mem, g.temperature),
                header: g.name,
                color: colors.gpu,
                chartLabel: qsTr("Utilization"),
                maximum: 100,
                maximumLabel: "100 %",
                series: [series(g.history, colors.gpu)],
                charts: [
                    mini(g.dedicated ? qsTr("Dedicated memory") : qsTr("Reserved memory"), mem ?? "", ch.memory, 100, colors.memory,
                         g.memory_total ? "" : qsTr("Not measured")),
                    mini(qsTr("GPU clock"), Format.mhz(g.core_mhz), ch.core_clock, g.core_max_mhz ?? 1, colors.clock,
                         g.core_mhz === null ? qsTr("Not measured") : ""),
                    mini(qsTr("Memory clock"), Format.mhz(g.memory_mhz), ch.memory_clock, g.memory_max_mhz ?? 1, colors.clock,
                         g.memory_mhz === null ? (g.dedicated ? qsTr("Not measured") : qsTr("Shared system memory")) : ""),
                    mini(qsTr("Voltage"), Format.millivolts(g.voltage_mv), ch.voltage, 1500, colors.voltage,
                         g.voltage_mv === null ? noVoltage : ""),
                    mini(qsTr("Power"), Format.watts(g.power_watts) + (g.power_limit_watts ? " / " + Format.watts(g.power_limit_watts) : ""),
                         ch.power, g.power_limit_watts ?? 1, colors.power, g.power_watts === null ? qsTr("Not measured") : ""),
                ],
                stats: [
                    [qsTr("Utilization"), Format.percent(g.usage)],
                    [qsTr("Dedicated memory"), mem ?? "—"],
                ].concat(g.shared_total ? [[qsTr("Shared memory"), Format.bytes(g.shared_used) + " / " + Format.bytes(g.shared_total)]] : [])
                 .concat(g.temperature !== null ? [[qsTr("Temperature"), Format.temperature(g.temperature)]] : [])
                 .concat(g.encoder !== null ? [[qsTr("Video encode"), Format.percent(g.encoder)]] : [])
                 .concat(g.decoder !== null ? [[qsTr("Video decode"), Format.percent(g.decoder)]] : []),
                details: details([
                    [qsTr("Type"), g.dedicated ? qsTr("Dedicated") : qsTr("Integrated")],
                    [qsTr("Driver"), g.driver],
                    [qsTr("Driver version"), g.driver_version],
                    [qsTr("Max clock"), g.core_max_mhz ? Format.mhz(g.core_max_mhz) : null],
                    [qsTr("PCI slot"), g.pci_slot],
                ]),
            }
        }
        case "sensors": {
            const all = [].concat(...s.sensors.map(g => g.sensors))
            const hottest = all.reduce((a, b) => b.value > a.value ? b : a)
            return {
                title: qsTr("Sensors"),
                subtitle: qsTr("%1 temperature sensors").arg(all.length),
                summary: qsTr("Max %1").arg(Format.temperature(hottest.value)),
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
                text: qsTr("60 seconds")
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
            title: qsTr("Performance")
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            QQC2.ScrollView {
                Layout.preferredWidth: Math.min(Kirigami.Units.gridUnit * 16, Math.max(Kirigami.Units.gridUnit * 11, page.width * 0.32))
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
                // Scrolls sideways instead of clipping once the content
                // reaches its minimum width.
                contentWidth: Math.max(availableWidth, Kirigami.Units.gridUnit * 22)

                ColumnLayout {
                    id: detail
                    readonly property var r: page.selected
                    readonly property real inner: width - Kirigami.Units.gridUnit * 2
                    visible: r !== null
                    width: detailScroll.contentWidth
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
                                    text: qsTr("% utilization per logical processor")
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

                                        QQC2.ToolTip.text: qsTr("Processor %1").arg(index)
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
                                columns: detail.inner > Kirigami.Units.gridUnit * 24 ? 2 : 1
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
            text: qsTr("Overall utilization")
            checkable: true
            checked: !page.logicalProcessors
            onTriggered: page.logicalProcessors = false
        }
        QQC2.MenuItem {
            visible: !!page.selected?.cores
            height: visible ? implicitHeight : 0
            text: qsTr("Logical processors")
            checkable: true
            checked: page.logicalProcessors
            onTriggered: page.logicalProcessors = true
        }
        QQC2.MenuSeparator {
            visible: !!page.selected?.cores
            height: visible ? implicitHeight : 0
        }
        QQC2.MenuItem {
            text: qsTr("Copy details")
            icon.source: Icons.of("copy")
            icon.color: Kirigami.Theme.textColor
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
