pragma Singleton

import QtCore
import QtQuick

// Saved settings; app.cpp reads language and theme ("general" is reserved in
// INI). Read and written one key at a time: Settings' own property binding
// saves every property when a key is missing, overwriting the ones it has not
// loaded yet with their defaults.
QtObject {
    id: prefs

    property string language: ""
    property int intervalMs: 1000
    property int startPage: 0
    property bool cpuPerCore: false
    property bool kernelTimes: false
    property int historyPoints: 60
    property bool smoothGraphs: false
    // Sizes in powers of 1000 (kB) instead of 1024.
    property bool sizeBase10: false
    property bool networkBytes: false
    property bool networkBase2: false
    // Network graphs scaled to the link speed instead of the peak seen.
    property bool networkScaleLink: false
    property bool dimZeros: false
    property bool columnSeparators: false
    property string sortColumn: "name"
    property bool sortDescending: false
    property bool showVirtualNetworks: false
    // Performance entries the user hid, and their order ("cpu,memory,...").
    property string hiddenResources: ""
    property string resourceOrder: ""
    property string theme: ""
    property bool miniPinned: false
    property string hiddenColumns: "threads,gpumem0,gpumem1,gpumem2,video"

    readonly property var keys: ["language", "intervalMs", "startPage", "cpuPerCore", "kernelTimes",
        "historyPoints", "smoothGraphs", "sizeBase10", "networkBytes", "networkBase2", "networkScaleLink",
        "dimZeros", "columnSeparators", "sortColumn", "sortDescending", "showVirtualNetworks",
        "hiddenResources", "resourceOrder", "theme", "miniPinned", "hiddenColumns"]

    property Settings store: Settings {
        category: "prefs"
    }

    // INI files give strings back: convert to the type of the default.
    function convert(stored, fallback) {
        switch (typeof fallback) {
        case "boolean": return stored === true || stored === "true"
        case "number": {
            const n = Number(stored)
            return isNaN(n) ? fallback : n
        }
        }
        return Array.isArray(stored) ? stored.map(s => String(s).trim()).join(",") : String(stored)
    }

    Component.onCompleted: {
        for (const key of keys) {
            const fallback = prefs[key]
            prefs[key] = convert(store.value(key, fallback), fallback)
            prefs[key + "Changed"].connect(() => store.setValue(key, prefs[key]))
        }
    }

    function isHidden(key) {
        return hiddenColumns.split(",").includes(key)
    }

    function toggleColumn(key) {
        const hidden = hiddenColumns.split(",")
        const list = hidden.filter(k => k !== "" && k !== key)
        hiddenColumns = (hidden.includes(key) ? list : list.concat([key])).join(",")
    }
}
