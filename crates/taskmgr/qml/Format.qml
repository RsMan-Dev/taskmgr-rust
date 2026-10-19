pragma Singleton

import QtQuick

// Number formatting shared by all pages, following the user's locale.
QtObject {
    function num(v, decimals) {
        return Number(v).toLocaleString(Qt.locale(), "f", decimals)
    }

    function percent(v) {
        return v === null || v === undefined ? "—" : num(v, 0) + " %"
    }

    // Binary units with the French "octet" symbols, like Windows does.
    function bytes(b) {
        const units = ["o", "Ko", "Mo", "Go", "To"]
        let i = 0
        while (b >= 1024 && i < units.length - 1) {
            b /= 1024
            i++
        }
        return num(b, i === 0 || b >= 100 ? 0 : 1) + " " + units[i]
    }

    function rate(bps) {
        return bytes(bps) + "/s"
    }

    // Network throughput in bits per second.
    function bits(bps) {
        let v = bps * 8
        const units = ["b/s", "kb/s", "Mb/s", "Gb/s"]
        let i = 0
        while (v >= 1000 && i < units.length - 1) {
            v /= 1000
            i++
        }
        return num(v, i === 0 || v >= 100 ? 0 : 1) + " " + units[i]
    }

    function ghz(mhz) {
        return mhz ? num(mhz / 1000, 2) + " GHz" : "—"
    }

    function temperature(c) {
        return c === null || c === undefined ? "—" : num(c, 0) + " °C"
    }

    // d:hh:mm:ss
    function uptime(secs) {
        const pad = n => String(n).padStart(2, "0")
        const d = Math.floor(secs / 86400)
        const h = Math.floor(secs % 86400 / 3600)
        const m = Math.floor(secs % 3600 / 60)
        return d + ":" + pad(h) + ":" + pad(m) + ":" + pad(Math.floor(secs % 60))
    }

    function niceMax(v, floor) {
        v = Math.max(v, floor)
        const p = Math.pow(10, Math.floor(Math.log10(v)))
        for (const m of [1, 2, 5, 10]) {
            if (m * p >= v) {
                return m * p
            }
        }
        return 10 * p
    }
}
