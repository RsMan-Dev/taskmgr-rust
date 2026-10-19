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
        efficiency: "#1abc9c",
    })

    // SMART health by block device name, from UDisks2.
    readonly property var health: monitor.health !== "" ? JSON.parse(monitor.health) : ({})
    readonly property var healthText: ({
        good: qsTr("Good"),
        warning: qsTr("Needs attention"),
        failing: qsTr("Failing"),
    })
    readonly property var healthReasons: ({
        "failing": qsTr("the drive predicts its own failure"),
        "attributes-failing": qsTr("an attribute is below its threshold"),
        "attributes-failed-past": qsTr("an attribute fell below its threshold in the past"),
        "overheated-past": qsTr("overheated in the past"),
        "bad-sectors": qsTr("bad sectors"),
        "critical-warning": qsTr("critical warning from the controller"),
        "spare-low": qsTr("spare blocks nearly exhausted"),
        "worn": qsTr("rated endurance nearly used up"),
        "media-errors": qsTr("media errors"),
    })

    readonly property var power: monitor.power !== "" ? JSON.parse(monitor.power) : null
    readonly property var profileNames: ({
        "power-saver": qsTr("Power saver"),
        "balanced": qsTr("Balanced"),
        "performance": qsTr("Performance"),
    })
    readonly property var batteryStatus: ({
        "Charging": qsTr("Charging"),
        "Discharging": qsTr("On battery"),
        "Full": qsTr("Full"),
        "Not charging": qsTr("Plugged in, not charging"),
    })

    property string selectedKey: "cpu"

    // The profile can change from the desktop's own controls.
    Timer {
        interval: 10000
        repeat: true
        triggeredOnStart: true
        running: page.visible
        onTriggered: page.monitor.refreshPower()
    }
    // CPU graph: "overall", "logical" (one per processor), "clocks" (MHz
    // per processor) or "numa".
    property string cpuView: "overall"

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
            .concat(s.batteries.map(b => "battery:" + b.name))
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

    // Element-wise mean of histories (right-aligned, they may differ in length).
    function average(histories) {
        const n = Math.max(0, ...histories.map(h => h.length))
        const out = []
        for (let i = 0; i < n; i++) {
            let sum = 0, k = 0
            for (const h of histories) {
                const v = h[h.length - n + i]
                if (v !== undefined) {
                    sum += v
                    k++
                }
            }
            out.push(k ? sum / k : 0)
        }
        return out
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
                clocks: c.core_mhz_history,
                maxMhz: c.max_mhz ?? Math.max(1, ...c.core_mhz_history.map(h => Math.max(0, ...h))),
                profile: page.power?.profile ? page.power : null,
                coreTypes: c.core_types,
                numa: c.numa.map(list => average(list.map(i => c.core_history[i] ?? []))),
                energy: true,
                charts: [
                    mini(qsTr("Temperature"), Format.temperature(c.temperature), ch.temperature, 100, colors.temperature,
                         c.temperature === null ? qsTr("No CPU temperature sensor") : ""),
                    mini(qsTr("Average clock"), Format.mhz(c.mhz), ch.clock, c.max_mhz ?? 1, colors.clock),
                    Object.assign(mini(c.voltage_source === "vid" ? qsTr("Voltage (requested)") : qsTr("Voltage"),
                         c.voltage !== null ? (c.voltage_source === "vid" ? "\u2248 " : "") + Format.num(c.voltage, 3) + " V" : "",
                         ch.voltage, 1.5, colors.voltage,
                         c.voltage === null ? qsTr("No driver exposes it.") : ""), {
                        note: c.voltage_source === "vid" ? qsTr("Approximate. More precise with %1 installed.").arg(
                            c.model.includes("AMD") ? "zenpower, it87 " + qsTr("or") + " nct6775" : "nct6775 " + qsTr("or") + " it87") : "",
                        actionText: c.voltage === null && page.monitor.networkAccess !== "active" ? qsTr("Estimate (root)…") : "",
                    }),
                ],
                stats: [
                    [qsTr("Utilization"), Format.percent(c.usage)],
                    [qsTr("Speed"), Format.ghz(c.mhz)],
                    [qsTr("Processes"), Format.num(c.processes, 0)],
                    [qsTr("Threads"), Format.num(c.threads, 0)],
                    [qsTr("Up time"), Format.uptime(c.uptime_secs)],
                    [qsTr("Temperature"), Format.temperature(c.temperature)],
                ].concat(s.energy.package_watts !== null ? [[qsTr("Power"), Format.watts(s.energy.package_watts)]] : [])
                 .concat(c.throttle_count !== null ? [[qsTr("Thermal throttling"), Format.num(c.throttle_count, 0)]] : []),
                details: details([
                    [qsTr("Max speed"), c.max_mhz ? Format.ghz(c.max_mhz) : null],
                    [qsTr("Sockets"), c.sockets],
                    [qsTr("Cores"), c.cores],
                    [qsTr("Logical processors"), c.logical],
                    [qsTr("Virtualization"), c.virtualization],
                    [qsTr("Frequency driver"), page.power?.driver],
                    [qsTr("Governor"), page.power?.governor],
                    [qsTr("Energy preference"), page.power?.epp],
                ].concat(c.caches.map(k => [qsTr("Cache %1").arg(k.name), Format.bytes(k.bytes)]))),
            }
        }
        case "battery": {
            const b = s.batteries.find(x => x.name === id)
            if (!b) {
                return null
            }
            const status = batteryStatus[b.status] ?? (b.on_ac ? qsTr("Plugged in") : qsTr("On battery"))
            const left = b.seconds_left !== null
                ? (b.status === "Charging" ? qsTr("%1 until full") : qsTr("%1 left")).arg(Format.duration(b.seconds_left)) : ""
            const topW = Format.niceMax(Math.max(...b.watts_history, 0), 5)
            return {
                title: qsTr("Battery"),
                subtitle: status,
                summary: summary(Format.percent(b.percent), left),
                header: b.model,
                color: colors.power,
                chartLabel: qsTr("Charge"),
                maximum: 100,
                maximumLabel: "100 %",
                series: [series(b.percent_history, colors.power)],
                secondChart: {
                    label: b.status === "Charging" ? qsTr("Charging power") : qsTr("Power draw"),
                    maximum: topW,
                    maximumLabel: Format.watts(topW),
                    series: [series(b.watts_history, colors.clock)],
                },
                stats: [
                    [qsTr("Charge"), Format.percent(b.percent)],
                    [qsTr("Status"), status],
                ].concat(left ? [[b.status === "Charging" ? qsTr("Until full") : qsTr("Time left"), Format.duration(b.seconds_left)]] : [])
                 .concat(b.watts !== null ? [[qsTr("Power"), Format.watts(b.watts)]] : []),
                details: details([
                    [qsTr("Energy"), b.energy_wh !== null && b.full_wh !== null ? qsTr("%1 / %2 Wh").arg(Format.num(b.energy_wh, 1)).arg(Format.num(b.full_wh, 1)) : null],
                    [qsTr("Design capacity"), b.design_wh !== null ? qsTr("%1 Wh").arg(Format.num(b.design_wh, 1)) : null],
                    [qsTr("Health"), b.full_wh !== null && b.design_wh ? Format.percent(b.full_wh / b.design_wh * 100) : null],
                    [qsTr("Charge cycles"), b.cycles],
                    [qsTr("Technology"), b.technology],
                ]),
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
                composition: { used: m.used, cached: m.cached, free: m.free, total: m.total },
                charts: m.pressure !== null ? [
                    mini(qsTr("Memory pressure"), Format.percent(m.pressure, 1), m.pressure_history, 10, colors.memory),
                ] : undefined,
                stats: [
                    [qsTr("In use"), Format.bytes(m.used)],
                    [qsTr("Available"), Format.bytes(m.available)],
                    [qsTr("Committed"), Format.bytes(m.committed)],
                    [qsTr("Cached"), Format.bytes(m.cached)],
                    [qsTr("Swap"), m.swap_total ? Format.bytes(m.swap_used) + " / " + Format.bytes(m.swap_total) : qsTr("None")],
                ].concat(m.zram_original ? [[qsTr("Compressed (zram)"), qsTr("%1 in %2").arg(Format.bytes(m.zram_original))
                    .arg(Format.bytes(m.zram_compressed))]] : [])
                 .concat(m.pressure !== null ? [[qsTr("Pressure"), Format.percent(m.pressure, 1)]] : []),
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
            const h = health[d.name] ?? null
            const temperature = d.temperature ?? h?.temperature ?? null
            const opt = (v, f) => v === null || v === undefined ? null : f(v)
            return {
                title: qsTr("Disk %1 (%2)").arg(d.index).arg(d.name),
                subtitle: Tr.core(d.kind) + (h && h.status !== "good" ? "  \u2022  " + healthText[h.status] : ""),
                summary: summary(Format.percent(d.active), Format.rate(d.read_bps + d.write_bps), temperature),
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
                ].concat(temperature !== null ? [[qsTr("Temperature"), Format.temperature(temperature)]] : [])
                 .concat(h ? [[qsTr("Health"), healthText[h.status]]] : []),
                details: details([
                    [qsTr("Capacity"), Format.bytes(d.capacity)],
                    [qsTr("Type"), Tr.core(d.kind)],
                    [qsTr("System disk"), d.system_disk ? qsTr("Yes") : qsTr("No")],
                    [qsTr("Mount points"), d.mounts.join(", ") || qsTr("None")],
                    [qsTr("Health"), h && h.reasons.length > 0 ? h.reasons.map(r => healthReasons[r] ?? r).join(", ") : null],
                    [qsTr("Powered on"), opt(h?.power_on_hours, v => qsTr("%1 h").arg(Format.num(v, 0)))],
                    [qsTr("Endurance used"), opt(h?.wear_percent, v => Format.percent(v))],
                    [qsTr("Spare blocks"), opt(h?.spare_percent, v => Format.percent(v))],
                    [qsTr("Data written"), opt(h?.written_bytes, v => Format.bytes(v))],
                    [qsTr("Bad sectors"), opt(h?.bad_sectors, v => Format.num(v, 0))],
                    [qsTr("Unsafe shutdowns"), opt(h?.unsafe_shutdowns, v => Format.num(v, 0))],
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
                    [qsTr("Received in total"), Format.bytes(n.rx_total)],
                    [qsTr("Sent in total"), Format.bytes(n.tx_total)],
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
                ].concat(g.encoder !== null ? [mini(qsTr("Video encode"), Format.percent(g.encoder), ch.encoder, 100, colors.gpu)] : [])
                 .concat(g.decoder !== null ? [mini(qsTr("Video decode"), Format.percent(g.decoder), ch.decoder, 100, colors.gpu)] : []),
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

                        RowLayout {
                            visible: !!detail.r?.profile
                            spacing: Kirigami.Units.largeSpacing
                            QQC2.Label {
                                text: qsTr("Power profile")
                            }
                            QQC2.ComboBox {
                                readonly property var profiles: detail.r?.profile?.profiles ?? []
                                model: profiles.map(p => page.profileNames[p] ?? p)
                                implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                                currentIndex: profiles.indexOf(detail.r?.profile?.profile ?? "")
                                onActivated: (i) => page.monitor.setPowerProfile(profiles[i])
                            }
                            QQC2.Label {
                                visible: (detail.r?.profile?.degraded ?? "") !== ""
                                text: qsTr("Performance held back: %1").arg(detail.r?.profile?.degraded ?? "")
                                opacity: 0.7
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                        }

                        SensorsView {
                            visible: !!detail.r?.sensors
                            groups: detail.r?.sensors ? page.snapshot.sensors : []
                            Layout.fillWidth: true
                        }

                        ChartBlock {
                            id: mainChart
                            visible: !detail.r?.sensors && !(detail.r?.cores && page.cpuView !== "overall")
                            chart: detail.r
                            accent: detail.r?.color ?? "gray"
                            chartHeight: Math.max(Kirigami.Units.gridUnit * 7, page.height * (detail.r?.secondChart || detail.r?.charts ? 0.3 : 0.42))
                            Layout.fillWidth: true
                            onContextMenuRequested: (position) => chartMenu.popup()
                        }

                        MemoryComposition {
                            visible: !!detail.r?.composition
                            composition: detail.r?.composition ?? null
                            accent: page.colors.memory
                            Layout.fillWidth: true
                        }

                        // Windows-style "logical processors" view of the CPU.
                        ColumnLayout {
                            visible: !!detail.r?.cores && page.cpuView !== "overall"
                            spacing: Kirigami.Units.smallSpacing / 2
                            Layout.fillWidth: true

                            RowLayout {
                                Caption {
                                    text: page.cpuView === "numa" ? qsTr("% utilization per NUMA node")
                                        : page.cpuView === "clocks" ? qsTr("Frequency per logical processor")
                                        : qsTr("% utilization per logical processor")
                                        + ((detail.r?.coreTypes?.length ?? 0) > 0 ? "  " + qsTr("(P: performance cores, E: efficiency cores in green)") : "")
                                    Layout.fillWidth: true
                                }
                                Caption {
                                    text: page.cpuView === "clocks" ? Format.ghz(detail.r?.maxMhz ?? 0) : "100 %"
                                }
                            }
                            GridLayout {
                                id: coreGrid
                                readonly property var lines: (page.cpuView === "numa" ? detail.r?.numa
                                    : page.cpuView === "clocks" ? detail.r?.clocks : detail.r?.cores) ?? []
                                readonly property int count: lines.length
                                columns: Math.max(1, Math.ceil(Math.sqrt(count * 2)))
                                columnSpacing: Kirigami.Units.smallSpacing
                                rowSpacing: Kirigami.Units.smallSpacing
                                Layout.fillWidth: true

                                Repeater {
                                    model: coreGrid.count
                                    UsageChart {
                                        required property int index
                                        compact: true
                                        readonly property bool efficient: page.cpuView !== "numa" && detail.r?.coreTypes?.[index] === "E"
                                        series: [{ values: coreGrid.lines[index] ?? [], color: accent }]
                                        maximum: page.cpuView === "clocks" ? detail.r?.maxMhz ?? 1 : 100
                                        accent: efficient ? page.colors.efficiency : page.cpuView === "clocks" ? page.colors.clock : page.colors.cpu
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: Math.max(Kirigami.Units.gridUnit * 2.5,
                                            page.height * 0.42 / Math.ceil(coreGrid.count / coreGrid.columns))

                                        QQC2.ToolTip.text: page.cpuView === "numa" ? qsTr("NUMA node %1").arg(index)
                                            : qsTr("Processor %1").arg(index) + (detail.r?.coreTypes?.[index] ? " (" + detail.r.coreTypes[index] + ")" : "")
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
                                    note: modelData.note ?? ""
                                    actionText: modelData.actionText ?? ""
                                    onActionTriggered: page.monitor.enableNetworkMonitor()
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
            checked: page.cpuView === "overall"
            onTriggered: page.cpuView = "overall"
        }
        QQC2.MenuItem {
            visible: !!page.selected?.cores
            height: visible ? implicitHeight : 0
            text: qsTr("Logical processors")
            checkable: true
            checked: page.cpuView === "logical"
            onTriggered: page.cpuView = "logical"
        }
        QQC2.MenuItem {
            visible: (page.selected?.clocks?.length ?? 0) > 0
            height: visible ? implicitHeight : 0
            text: qsTr("Frequency per processor")
            checkable: true
            checked: page.cpuView === "clocks"
            onTriggered: page.cpuView = "clocks"
        }
        QQC2.MenuItem {
            visible: (page.selected?.numa?.length ?? 0) > 1
            height: visible ? implicitHeight : 0
            text: qsTr("NUMA nodes")
            checkable: true
            checked: page.cpuView === "numa"
            onTriggered: page.cpuView = "numa"
        }
        QQC2.MenuSeparator {
            visible: !!page.selected?.cores
            height: visible ? implicitHeight : 0
        }
        QQC2.MenuItem {
            text: qsTr("Copy details")
            icon.source: Icons.of("copy", Kirigami.Theme.textColor)
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
