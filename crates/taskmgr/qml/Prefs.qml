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
