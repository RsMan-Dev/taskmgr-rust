pragma Singleton

import QtQuick

// Translates the English strings produced by the Rust core.
QtObject {
    readonly property var fixed: [
        QT_TRANSLATE_NOOP("core", "Login session"),
        QT_TRANSLATE_NOOP("core", "Docker container"),
        QT_TRANSLATE_NOOP("core", "Kernel"),
        QT_TRANSLATE_NOOP("core", "CPU"),
        QT_TRANSLATE_NOOP("core", "Application"),
        QT_TRANSLATE_NOOP("core", "Motherboard"),
        QT_TRANSLATE_NOOP("core", "ACPI (thermal zone)"),
        QT_TRANSLATE_NOOP("core", "Tctl (control)"),
        QT_TRANSLATE_NOOP("core", "Die"),
        QT_TRANSLATE_NOOP("core", "Package"),
        QT_TRANSLATE_NOOP("core", "Edge"),
        QT_TRANSLATE_NOOP("core", "Junction"),
        QT_TRANSLATE_NOOP("core", "Memory"),
        QT_TRANSLATE_NOOP("core", "SD card"),
        QT_TRANSLATE_NOOP("core", "Optical"),
        QT_TRANSLATE_NOOP("core", "pkexec not found (polkit package)."),
        QT_TRANSLATE_NOOP("core", "chown not found."),
        QT_TRANSLATE_NOOP("core", "No RAPL counter on this machine."),
        QT_TRANSLATE_NOOP("core", "Authorization denied or cancelled."),
        QT_TRANSLATE_NOOP("core", "renice not found."),
        QT_TRANSLATE_NOOP("core", "Invalid unit name."),
        QT_TRANSLATE_NOOP("core", "taskmgr-helper not found."),
        QT_TRANSLATE_NOOP("core", "Utilization"),
        QT_TRANSLATE_NOOP("core", "Average clock"),
        QT_TRANSLATE_NOOP("core", "Package power"),
        QT_TRANSLATE_NOOP("core", "Cores power"),
        QT_TRANSLATE_NOOP("core", "Voltage"),
        QT_TRANSLATE_NOOP("core", "In use"),
        QT_TRANSLATE_NOOP("core", "Swap used"),
        QT_TRANSLATE_NOOP("core", "Clock"),
        QT_TRANSLATE_NOOP("core", "Memory clock"),
        QT_TRANSLATE_NOOP("core", "Power"),
        QT_TRANSLATE_NOOP("core", "Fan"),
        QT_TRANSLATE_NOOP("core", "Memory used"),
        QT_TRANSLATE_NOOP("core", "Active time"),
        QT_TRANSLATE_NOOP("core", "Read"),
        QT_TRANSLATE_NOOP("core", "Write"),
        QT_TRANSLATE_NOOP("core", "Receive"),
        QT_TRANSLATE_NOOP("core", "Send"),
        QT_TRANSLATE_NOOP("core", "Charge"),
        QT_TRANSLATE_NOOP("core", "Composite"),
        QT_TRANSLATE_NOOP("core", "GPU"),
        QT_TRANSLATE_NOOP("core", "Not enough free space in this folder."),
        QT_TRANSLATE_NOOP("core", "Stopped."),
        QT_TRANSLATE_NOOP("core", "Unknown test."),
        QT_TRANSLATE_NOOP("core", "journalctl not found: no systemd journal."),
    ]

    readonly property var patterns: [
        [/^Sensor (.+)$/, QT_TRANSLATE_NOOP("core", "Sensor %1")],
        [/^Processor (\d+)$/, QT_TRANSLATE_NOOP("core", "Processor %1")],
        [/^Disk (\d+) \((.*)\)$/, QT_TRANSLATE_NOOP("core", "Disk %1 (%2)")],
        [/^Network card (.+)$/, QT_TRANSLATE_NOOP("core", "Network card %1")],
        [/^Mount (.+)$/, QT_TRANSLATE_NOOP("core", "Mount %1")],
        [/^Docker container \((.+)\)$/, QT_TRANSLATE_NOOP("core", "Docker container (%1)")],
        [/^The command failed \((.*)\)\.$/, QT_TRANSLATE_NOOP("core", "The command failed (%1).")],
        [/^(.+) \(removable\)$/, QT_TRANSLATE_NOOP("core", "%1 (removable)")],
    ]

    function core(s) {
        if (!s) {
            return ""
        }
        for (const [re, template] of patterns) {
            const m = s.match(re)
            if (m) {
                let out = qsTranslate("core", template)
                for (let i = 1; i < m.length; i++) {
                    out = out.arg(core(m[i]))
                }
                return out
            }
        }
        return qsTranslate("core", s)
    }
}
