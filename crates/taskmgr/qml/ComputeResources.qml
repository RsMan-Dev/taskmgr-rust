import QtQuick
import org.kde.kirigami as Kirigami

// CPU, power, battery and memory, as shown by the Performance page.
QtObject {
    id: root

    property var monitor: null
    property var colors: ({})
    property string cpuView: "overall"
    readonly property var power: monitor && monitor.power !== "" ? JSON.parse(monitor.power) : null

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

    // Small charts per processor: usage, clock or NUMA node.
    function processorGrid(c, view) {
        const clocks = view === "clocks"
        return {
            lines: view === "numa" ? c.numa.map(list => ResourceKit.average(list.map(i => c.core_history[i] ?? [])))
                 : clocks ? c.core_mhz_history : c.core_history,
            maximum: clocks ? cpuMaxMhz(c) : 100,
            maximumLabel: clocks ? Format.ghz(cpuMaxMhz(c)) : "100 %",
            caption: view === "numa" ? qsTr("% utilization per NUMA node")
                : clocks ? qsTr("Frequency per logical processor")
                : qsTr("% utilization per logical processor")
                  + (c.core_types.length > 0 ? "  " + qsTr("(P: performance cores, E: efficiency cores in green)") : ""),
            accent: clocks ? colors.clock : colors.cpu,
            types: view === "numa" ? [] : c.core_types,
            numa: view === "numa",
        }
    }

    function cpuMaxMhz(c) {
        return c.max_mhz ?? Math.max(1, ...c.core_mhz_history.map(h => Math.max(0, ...h)))
    }

    function cpu(s) {
    const c = s.cpu
    const ch = c.charts
    return {
        title: qsTr("CPU"),
        subtitle: "",
        summary: ResourceKit.summary(Format.percent(c.usage), Format.ghz(c.mhz), c.temperature),
        header: c.model,
        color: colors.cpu,
        chartLabel: qsTr("% utilization"),
        maximum: 100,
        maximumLabel: "100 %",
        series: [ResourceKit.series(c.history, colors.cpu)],
        cpu: true,
        hasClocks: c.core_mhz_history.length > 0,
        hasNuma: c.numa.length > 1,
        grid: cpuView !== "overall" ? processorGrid(c, cpuView) : null,
        gridReplacesChart: true,
        energy: true,
        charts: [
            ResourceKit.mini(qsTr("Temperature"), Format.temperature(c.temperature), ch.temperature, 100, colors.temperature,
                 c.temperature === null ? qsTr("No CPU temperature sensor") : ""),
            ResourceKit.mini(qsTr("Average clock"), Format.mhz(c.mhz), ch.clock, c.max_mhz ?? 1, colors.clock),
            Object.assign(ResourceKit.mini(c.voltage_source === "vid" ? qsTr("Voltage (requested)") : qsTr("Voltage"),
                 c.voltage !== null ? (c.voltage_source === "vid" ? "\u2248 " : "") + Format.num(c.voltage, 3) + " V" : "",
                 ch.voltage, 1.5, colors.voltage,
                 c.voltage === null ? qsTr("No driver exposes it.") : ""), {
                note: c.voltage_source === "vid" ? qsTr("Approximate. More precise with %1 installed.").arg(
                    c.model.includes("AMD") ? "zenpower, it87 " + qsTr("or") + " nct6775" : "nct6775 " + qsTr("or") + " it87") : "",
                actionText: c.voltage === null && root.monitor.networkAccess !== "active" ? qsTr("Estimate (root)…") : "",
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
        details: ResourceKit.details([
            [qsTr("Max speed"), c.max_mhz ? Format.ghz(c.max_mhz) : null],
            [qsTr("Sockets"), c.sockets],
            [qsTr("Cores"), c.cores],
            [qsTr("Logical processors"), c.logical],
            [qsTr("Virtualization"), c.virtualization],
        ].concat(c.caches.map(k => [qsTr("Cache %1").arg(k.name), Format.bytes(k.bytes)]))),
    }
    }

    readonly property var sourceNames: ({
        "cpu": qsTr("CPU package"),
        "cpu-cores": qsTr("CPU cores"),
        "cpu-uncore": qsTr("Integrated graphics and cache"),
        "memory": qsTr("Memory"),
        "platform": qsTr("Whole machine (platform)"),
        "battery": qsTr("Battery discharge"),
    })

    function sourceName(src) {
        return src.kind === "gpu" || src.kind === "sensor" ? src.name : sourceNames[src.kind] ?? src.name
    }

    // Everything that reports the power it draws.
    function consumption(s) {
        const c = s.consumption
        const cpu = c.sources.find(x => x.kind === "cpu")
        const top = Format.niceMax(Math.max(...c.total_history, 0), 50)
        const totalLabel = {
            platform: qsTr("Whole machine"),
            battery: qsTr("On battery"),
            sum: qsTr("Measured parts"),
        }[c.total_kind] ?? ""
        const wh = v => qsTr("%1 Wh").arg(Format.num(v, v < 10 ? 2 : 1))
        return {
            title: qsTr("Power use"),
            subtitle: totalLabel,
            summary: ResourceKit.summary(c.total_watts !== null ? Format.watts(c.total_watts) : qsTr("Not measured"), ""),
            header: totalLabel,
            color: colors.power,
            chartLabel: cpu ? qsTr("Total power (CPU in blue)") : qsTr("Total power"),
            maximum: top,
            maximumLabel: Format.watts(top),
            series: [ResourceKit.series(c.total_history, colors.power), ResourceKit.series(cpu?.history ?? [], colors.cpu)],
            energy: true,
            charts: c.sources.map(src => ResourceKit.mini(sourceName(src), Format.watts(src.watts), src.history,
                Format.niceMax(Math.max(...src.history, 0), 5), src.kind === "gpu" ? colors.gpu : src.kind.startsWith("cpu") ? colors.cpu : colors.power)),
            stats: [
                [qsTr("Total"), c.total_watts !== null ? Format.watts(c.total_watts) : "—"],
                [qsTr("Since this window opened"), wh(c.total_wh)],
            ],
            details: ResourceKit.details(c.sources.map(src => [sourceName(src) + (src.part_of ? " " + qsTr("(included above)") : ""),
                    (src.watts !== null ? Format.watts(src.watts) : "—") + "  \u00b7  " + wh(src.energy_wh)])
                .concat([[qsTr("Disks, memory modules, fans"), qsTr("Not published by the hardware")]])),
        }
    }

    function battery(s, id) {
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
        summary: ResourceKit.summary(Format.percent(b.percent), left),
        header: b.model,
        color: colors.power,
        chartLabel: qsTr("Charge"),
        maximum: 100,
        maximumLabel: "100 %",
        series: [ResourceKit.series(b.percent_history, colors.power)],
        secondChart: {
            label: b.status === "Charging" ? qsTr("Charging power") : qsTr("Power draw"),
            maximum: topW,
            maximumLabel: Format.watts(topW),
            series: [ResourceKit.series(b.watts_history, colors.clock)],
        },
        stats: [
            [qsTr("Charge"), Format.percent(b.percent)],
            [qsTr("Status"), status],
        ].concat(left ? [[b.status === "Charging" ? qsTr("Until full") : qsTr("Time left"), Format.duration(b.seconds_left)]] : [])
         .concat(b.watts !== null ? [[qsTr("Power"), Format.watts(b.watts)]] : []),
        details: ResourceKit.details([
            [qsTr("Energy"), b.energy_wh !== null && b.full_wh !== null ? qsTr("%1 / %2 Wh").arg(Format.num(b.energy_wh, 1)).arg(Format.num(b.full_wh, 1)) : null],
            [qsTr("Design capacity"), b.design_wh !== null ? qsTr("%1 Wh").arg(Format.num(b.design_wh, 1)) : null],
            [qsTr("Health"), b.full_wh !== null && b.design_wh ? Format.percent(b.full_wh / b.design_wh * 100) : null],
            [qsTr("Charge cycles"), b.cycles],
            [qsTr("Technology"), b.technology],
        ]),
    }
    }

    function memory(s) {
    const m = s.memory
    const speed = m.dimms.length > 0 ? m.dimms[0].speed_mts : null
    return {
        title: qsTr("Memory"),
        subtitle: m.dimms.length > 0 && m.dimms[0].kind ? m.dimms[0].kind + (speed ? " " + qsTr("%1 MT/s").arg(speed) : "") : "",
        summary: ResourceKit.summary(Format.percent(m.usage), Format.bytes(m.used) + " / " + Format.bytes(m.total)),
        header: Format.bytes(m.total),
        color: colors.memory,
        chartLabel: qsTr("Memory usage"),
        maximum: 100,
        maximumLabel: Format.bytes(m.total),
        series: [ResourceKit.series(m.history, colors.memory)],
        dimms: m.dimms,
        composition: { used: m.used, cached: m.cached, free: m.free, total: m.total },
        charts: m.pressure !== null ? [
            ResourceKit.mini(qsTr("Memory pressure"), Format.percent(m.pressure, 1), m.pressure_history, 10, colors.memory),
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
        details: ResourceKit.details([
            [qsTr("Speed"), speed ? qsTr("%1 MT/s").arg(speed) : null],
            [qsTr("Type"), m.dimms.length > 0 ? m.dimms[0].kind : null],
            [qsTr("Slots used"), m.slots ? qsTr("%1 of %2").arg(m.dimms.length).arg(m.slots) : null],
            [qsTr("Usable"), Format.bytes(m.total)],
        ]),
    }
    }
}
