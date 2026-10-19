pragma Singleton

import QtCore
import QtQuick

// Saved settings; app.cpp reads language and theme ("general" is reserved in INI).
Settings {
    category: "prefs"

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

    function isHidden(key) {
        return hiddenColumns.split(",").includes(key)
    }

    function toggleColumn(key) {
        const hidden = hiddenColumns.split(",")
        const list = hidden.filter(k => k !== "" && k !== key)
        hiddenColumns = (hidden.includes(key) ? list : list.concat([key])).join(",")
    }
}
