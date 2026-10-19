pragma Singleton

import QtCore
import QtQuick

// Settings shared by every page, saved across runs. `language` and `theme`
// are also read by app.cpp before the interface loads. ("general" would
// clash with the INI format's reserved [General] section.)
Settings {
    category: "prefs"

    property string language: ""
    property int intervalMs: 1000
    property int startPage: 0
    property bool cpuPerCore: false
    // "", "light" or "dark"; read by app.cpp at start.
    property string theme: ""
    property bool miniPinned: false
    property string hiddenColumns: "threads,gpumem0,gpumem1,gpumem2"

    function isHidden(key) {
        return hiddenColumns.split(",").includes(key)
    }

    function toggleColumn(key) {
        const hidden = hiddenColumns.split(",")
        const list = hidden.filter(k => k !== "" && k !== key)
        hiddenColumns = (hidden.includes(key) ? list : list.concat([key])).join(",")
    }
}
