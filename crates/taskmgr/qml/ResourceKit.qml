pragma Singleton

import QtQuick

// Pure helpers for the resource descriptions of the Performance page.
QtObject {
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

    // Histories are right-aligned: they may differ in length.
    function sum(histories) {
        const n = Math.max(0, ...histories.map(h => h.length))
        const out = []
        for (let i = 0; i < n; i++) {
            out.push(histories.reduce((a, h) => a + (h[h.length - n + i] ?? 0), 0))
        }
        return out
    }

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

    function optional(v, format) {
        return v === null || v === undefined ? null : format(v)
    }
}
