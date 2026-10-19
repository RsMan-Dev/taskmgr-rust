import QtQuick

// Disks, network adapters, GPUs and sensors, as shown by the Performance page.
QtObject {
    id: root

    property var monitor: null
    property var colors: ({})

    // SMART health by block device name, from UDisks2.
    readonly property var health: monitor && monitor.health !== "" ? JSON.parse(monitor.health) : ({})
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

    function disk(s, id) {
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
        summary: ResourceKit.summary(Format.percent(d.active), Format.rate(d.read_bps + d.write_bps), temperature),
        header: d.model,
        color: colors.disk,
        chartLabel: qsTr("Active time (at least one request in flight)"),
        maximum: 100,
        maximumLabel: "100 %",
        series: [ResourceKit.series(d.active_history, colors.disk)],
        secondChart: {
            label: qsTr("Transfer rate (read, write)"),
            maximum: top,
            maximumLabel: Format.rate(top),
            series: [ResourceKit.series(d.read_history, colors.disk), ResourceKit.series(d.write_history, Qt.darker(colors.disk, 1.6))],
        },
        stats: [
            [qsTr("Active time"), Format.percent(d.active)],
            [qsTr("Average response time"), qsTr("%1 ms").arg(Format.num(d.response_ms, 1))],
            [qsTr("Read speed"), Format.rate(d.read_bps)],
            [qsTr("Write speed"), Format.rate(d.write_bps)],
        ].concat(temperature !== null ? [[qsTr("Temperature"), Format.temperature(temperature)]] : [])
         .concat(h ? [[qsTr("Health"), healthText[h.status]]] : []),
        actions: (h ? [{ id: "smart:" + d.name, text: qsTr("SMART details…"), icon: "details", label: d.model }] : [])
            .concat(d.kind.includes("(USB)") || d.kind.includes("(removable)") || /^(sr|mmcblk)/.test(d.name)
                    ? [{ id: "eject:" + d.name, text: qsTr("Eject"), icon: "eject", label: d.model }] : []),
        details: ResourceKit.details([
            [qsTr("Capacity"), Format.bytes(d.capacity)],
            [qsTr("Type"), Tr.core(d.kind)],
            [qsTr("System disk"), d.system_disk ? qsTr("Yes") : qsTr("No")],
            [qsTr("Mount points"), d.mounts.join(", ") || qsTr("None")],
            [qsTr("Health"), h && h.reasons.length > 0 ? h.reasons.map(r => healthReasons[r] ?? r).join(", ") : null],
            [qsTr("Powered on"), ResourceKit.optional(h?.power_on_hours, v => qsTr("%1 h").arg(Format.num(v, 0)))],
            [qsTr("Endurance used"), ResourceKit.optional(h?.wear_percent, v => Format.percent(v))],
            [qsTr("Spare blocks"), ResourceKit.optional(h?.spare_percent, v => Format.percent(v))],
            [qsTr("Data written"), ResourceKit.optional(h?.written_bytes, v => Format.bytes(v))],
            [qsTr("Bad sectors"), ResourceKit.optional(h?.bad_sectors, v => Format.num(v, 0))],
            [qsTr("Unsafe shutdowns"), ResourceKit.optional(h?.unsafe_shutdowns, v => Format.num(v, 0))],
        ]),
    }
    }

    function net(s, id) {
    const n = s.networks.find(x => x.name === id)
    if (!n) {
        return null
    }
    const rxTop = Format.niceMax(Math.max(...n.rx_history, 0), 128 * 1024)
    const txTop = Format.niceMax(Math.max(...n.tx_history, 0), 128 * 1024)
    const w = n.wireless
    const linkMbps = n.link_mbps ?? (w?.bitrate_kbps ? w.bitrate_kbps / 1000 : null)
    const load = linkMbps ? Math.max(n.rx_bps, n.tx_bps) * 8 / (linkMbps * 1e6) * 100 : null
    return {
        title: n.kind,
        subtitle: w?.ssid ?? n.name,
        summary: ResourceKit.summary(load !== null ? Format.percent(load) : "", qsTr("R %1  S %2").arg(Format.bits(n.rx_bps)).arg(Format.bits(n.tx_bps))),
        header: n.adapter,
        color: colors.network,
        chartLabel: qsTr("Receive"),
        maximum: rxTop,
        maximumLabel: Format.bits(rxTop),
        series: [ResourceKit.series(n.rx_history, colors.network)],
        secondChart: {
            label: qsTr("Send"),
            maximum: txTop,
            maximumLabel: Format.bits(txTop),
            series: [ResourceKit.series(n.tx_history, Qt.darker(colors.network, 1.4))],
        },
        stats: [
            [qsTr("Receive"), Format.bits(n.rx_bps)],
            [qsTr("Send"), Format.bits(n.tx_bps)],
            [qsTr("Received in total"), Format.bytes(n.rx_total)],
            [qsTr("Sent in total"), Format.bytes(n.tx_total)],
        ].concat(load !== null ? [[qsTr("Link utilization"), Format.percent(load)]] : []),
        details: ResourceKit.details([
            [qsTr("Adapter"), n.adapter],
            [qsTr("Interface"), n.name],
            [qsTr("Connection type"), n.kind],
            [qsTr("Link speed"), linkMbps ? Format.link(linkMbps) : null],
            [qsTr("Network name (SSID)"), w?.ssid],
            [qsTr("Signal strength"), w?.signal_percent != null ? Format.percent(w.signal_percent) : null],
            [qsTr("Frequency"), w?.frequency_mhz ? qsTr("%1 GHz").arg(Format.num(w.frequency_mhz / 1000, 2)) : null],
            [qsTr("IPv4 address"), n.ipv4.join("\n")],
            [qsTr("IPv6 address"), n.ipv6.join("\n")],
            [qsTr("MAC address"), n.mac],
        ]),
    }
    }

    function gpu(s, id) {
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
        summary: ResourceKit.summary(g.usage !== null ? Format.percent(g.usage) : qsTr("Not measured"), mem, g.temperature),
        header: g.name,
        color: colors.gpu,
        chartLabel: qsTr("Utilization"),
        maximum: 100,
        maximumLabel: "100 %",
        series: [ResourceKit.series(g.history, colors.gpu)],
        charts: [
            ResourceKit.mini(g.dedicated ? qsTr("Dedicated memory") : qsTr("Reserved memory"), mem ?? "", ch.memory, 100, colors.memory,
                 g.memory_total ? "" : qsTr("Not measured")),
            ResourceKit.mini(qsTr("GPU clock"), Format.mhz(g.core_mhz), ch.core_clock, g.core_max_mhz ?? 1, colors.clock,
                 g.core_mhz === null ? qsTr("Not measured") : ""),
            ResourceKit.mini(qsTr("Memory clock"), Format.mhz(g.memory_mhz), ch.memory_clock, g.memory_max_mhz ?? 1, colors.clock,
                 g.memory_mhz === null ? (g.dedicated ? qsTr("Not measured") : qsTr("Shared system memory")) : ""),
            ResourceKit.mini(qsTr("Voltage"), Format.millivolts(g.voltage_mv), ch.voltage, 1500, colors.voltage,
                 g.voltage_mv === null ? noVoltage : ""),
            ResourceKit.mini(qsTr("Power"), Format.watts(g.power_watts) + (g.power_limit_watts ? " / " + Format.watts(g.power_limit_watts) : ""),
                 ch.power, g.power_limit_watts ?? 1, colors.power, g.power_watts === null ? qsTr("Not measured") : ""),
        ].concat(g.encoder !== null ? [ResourceKit.mini(qsTr("Video encode"), Format.percent(g.encoder), ch.encoder, 100, colors.gpu)] : [])
         .concat(g.decoder !== null ? [ResourceKit.mini(qsTr("Video decode"), Format.percent(g.decoder), ch.decoder, 100, colors.gpu)] : []),
        stats: [
            [qsTr("Utilization"), Format.percent(g.usage)],
            [qsTr("Dedicated memory"), mem ?? "—"],
        ].concat(g.shared_total ? [[qsTr("Shared memory"), Format.bytes(g.shared_used) + " / " + Format.bytes(g.shared_total)]] : [])
         .concat(g.temperature !== null ? [[qsTr("Temperature"), Format.temperature(g.temperature)]] : [])
         .concat(g.encoder !== null ? [[qsTr("Video encode"), Format.percent(g.encoder)]] : [])
         .concat(g.decoder !== null ? [[qsTr("Video decode"), Format.percent(g.decoder)]] : []),
        details: ResourceKit.details([
            [qsTr("Type"), g.dedicated ? qsTr("Dedicated") : qsTr("Integrated")],
            [qsTr("Driver"), g.driver],
            [qsTr("Driver version"), g.driver_version],
            [qsTr("Vulkan"), g.vulkan],
            [qsTr("Max clock"), g.core_max_mhz ? Format.mhz(g.core_max_mhz) : null],
            [qsTr("Bus"), g.pcie_now ? (g.pcie_max && g.pcie_max !== g.pcie_now
                ? qsTr("%1 (maximum %2)").arg(g.pcie_now).arg(g.pcie_max) : g.pcie_now) : null],
            [qsTr("PCI slot"), g.pci_slot],
        ]),
    }
    }

    function fan(s, id) {
    const f = s.fans.find(x => x.key === id)
    if (!f) {
        return null
    }
    const top = f.max_rpm ?? Format.niceMax(Math.max(...f.history, 0), 1000)
    return {
        title: qsTr("Fan"),
        subtitle: Tr.core(f.name),
        summary: ResourceKit.summary(f.percent !== null ? Format.percent(f.percent) : "",
                                     f.rpm !== null ? qsTr("%1 RPM").arg(Format.num(f.rpm, 0)) : ""),
        header: Tr.core(f.source),
        color: colors.fan,
        chartLabel: qsTr("Speed"),
        maximum: top,
        maximumLabel: qsTr("%1 RPM").arg(Format.num(top, 0)),
        series: [ResourceKit.series(f.history, colors.fan)],
        secondChart: f.percent !== null ? {
            label: qsTr("Duty cycle (PWM)"),
            maximum: 100,
            maximumLabel: "100 %",
            series: [ResourceKit.series(f.percent_history, Qt.darker(colors.fan, 1.4))],
        } : null,
        stats: [
            [qsTr("Speed"), f.rpm !== null ? qsTr("%1 RPM").arg(Format.num(f.rpm, 0)) : "—"],
        ].concat(f.percent !== null ? [[qsTr("Duty cycle"), Format.percent(f.percent)]] : []),
        details: ResourceKit.details([
            [qsTr("Maximum speed"), f.max_rpm ? qsTr("%1 RPM").arg(Format.num(f.max_rpm, 0)) : null],
            [qsTr("Sensor"), f.key],
        ]),
    }
    }

    function sensors(s) {
    const all = [].concat(...s.sensors.map(g => g.sensors))
    const hottest = all.reduce((a, b) => b.value > a.value ? b : a)
    return {
        title: qsTr("Sensors"),
        subtitle: qsTr("%1 temperature sensors").arg(all.length),
        summary: qsTr("Max %1").arg(Format.temperature(hottest.value)),
        header: "",
        color: colors.sensors,
        maximum: 110,
        series: [ResourceKit.series(hottest.history, colors.sensors)],
        sensors: true,
    }
    }
}
