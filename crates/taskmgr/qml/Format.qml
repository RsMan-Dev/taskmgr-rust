pragma Singleton

import QtQuick

// Number formatting shared by all pages, following the user's locale.
QtObject {
    function num(v, decimals) {
        return Number(v).toLocaleString(Qt.locale(), "f", decimals)
    }

    function percent(v, decimals) {
        return v === null || v === undefined ? "—" : qsTr("%1%").arg(num(v, decimals ?? 0))
    }

    // Binary units, like Windows does (octet symbols in French).
    function bytes(b) {
        const units = [qsTr("B"), qsTr("KB"), qsTr("MB"), qsTr("GB"), qsTr("TB")]
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

    function mhz(v) {
        if (v === null || v === undefined) {
            return "—"
        }
        return v >= 1000 ? num(v / 1000, 2) + " GHz" : num(v, 0) + " MHz"
    }

    function watts(v) {
        return v === null || v === undefined ? "—" : num(v, v < 10 ? 1 : 0) + " W"
    }

    function millivolts(v) {
        return v === null || v === undefined ? "—" : num(v / 1000, 3) + " V"
    }

    function link(mbps) {
        return mbps >= 1000 ? num(mbps / 1000, 1) + " Gb/s" : mbps + " Mb/s"
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

    // Duration, "2 h 05", "35 min" or "40 s".
    function duration(secs) {
        const h = Math.floor(secs / 3600)
        const m = Math.floor(secs % 3600 / 60)
        if (h === 0 && m === 0) {
            return qsTr("%1 s").arg(Math.floor(secs))
        }
        return h > 0 ? qsTr("%1 h %2").arg(h).arg(String(m).padStart(2, "0")) : qsTr("%1 min").arg(m)
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
