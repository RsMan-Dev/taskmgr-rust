pragma Singleton

import QtCore
import QtQuick

// Settings shared by every page, saved across runs. `language` is also read
// by app.cpp before the interface loads, hence the "general" category.
Settings {
    category: "general"

    property string language: ""
    property int intervalMs: 1000
    property int startPage: 0
    property bool cpuPerCore: false
    property string hiddenColumns: "threads,gpumem0,gpumem1,gpumem2"

    readonly property var hiddenList: hiddenColumns.split(",")

    function toggleColumn(key) {
        const list = hiddenList.filter(k => k !== "" && k !== key)
        hiddenColumns = (hiddenList.includes(key) ? list : list.concat([key])).join(",")
    }
}
