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
    ]

    readonly property var patterns: [
        [/^Sensor (.+)$/, QT_TRANSLATE_NOOP("core", "Sensor %1")],
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
