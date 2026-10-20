import QtQuick
import QtQuick.Layouts

// Newest value of a benchmark figure, with the change since the run before.
DetailRow {
    property var values: []
    property var text: v => String(v)
    property bool lowerIsBetter: false

    function change(now, before) {
        if (!before || !now) {
            return ""
        }
        const pct = (now - before) / before * 100 * (lowerIsBetter ? -1 : 1)
        return (pct >= 0 ? "+" : "") + Format.num(pct, 1) + " %"
    }

    visible: values.length > 0
    value: values.length > 0 ? text(values[0]) + "   " + change(values[0], values[1]) : ""
    Layout.fillWidth: true
}
