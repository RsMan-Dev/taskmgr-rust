import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Every SMART attribute of a drive and its last self-test.
QQC2.Dialog {
    id: dialog

    required property var monitor
    readonly property var details: monitor.smartDetails !== "" ? JSON.parse(monitor.smartDetails) : null
    property bool loading: false

    readonly property var selftestText: ({
        "success": qsTr("Passed"),
        "aborted": qsTr("Aborted"),
        "interrupted": qsTr("Interrupted"),
        "fatal": qsTr("Failed"),
        "inprogress": qsTr("In progress"),
    })
    readonly property var stateText: ({
        "ok": qsTr("OK"),
        "failing": qsTr("Failing"),
        "failed-past": qsTr("Failed in the past"),
    })

    function show(block, name) {
        title = qsTr("SMART: %1").arg(name)
        loading = true
        monitor.requestSmartDetails(block)
        open()
    }

    // "reallocated-sector-count" to "Reallocated sector count".
    function pretty(name) {
        const s = name.replace(/[-_]/g, " ")
        return s.charAt(0).toUpperCase() + s.slice(1)
    }

    parent: QQC2.Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(parent.width - Kirigami.Units.gridUnit * 2, Kirigami.Units.gridUnit * 50)
    height: Math.min(parent.height - Kirigami.Units.gridUnit * 3, Kirigami.Units.gridUnit * 34)
    modal: true
    standardButtons: QQC2.Dialog.Close

    Connections {
        target: dialog.monitor
        function onSmartDetailsChanged() {
            if (dialog.monitor.smartDetails !== "") {
                dialog.loading = false
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        QQC2.Label {
            visible: !!dialog.details?.selftest
            text: qsTr("Last self-test: %1").arg(dialog.selftestText[dialog.details?.selftest] ?? dialog.details?.selftest ?? "")
                + (dialog.details?.selftest === "inprogress" && dialog.details?.selftest_remaining !== null
                   ? "  " + qsTr("(%1 left)").arg(Format.percent(dialog.details.selftest_remaining)) : "")
        }

        DataTable {
            rows: dialog.details?.attributes ?? []
            keyOf: r => r.name
            ordered: true
            placeholder: dialog.loading ? qsTr("Reading…") : qsTr("No SMART data for this drive.")
            Layout.fillWidth: true
            Layout.fillHeight: true
            columns: (dialog.details?.kind === "ata" ? [
                { key: "id", title: qsTr("ID"), align: Text.AlignRight, text: r => String(r.id) },
            ] : []).concat([
                { key: "name", title: qsTr("Attribute"), width: 0, text: r => dialog.pretty(r.name) },
            ]).concat(dialog.details?.kind === "ata" ? [
                { key: "value", title: qsTr("Value"), align: Text.AlignRight, text: r => r.value ?? "" },
                { key: "worst", title: qsTr("Worst"), align: Text.AlignRight, text: r => r.worst ?? "" },
                { key: "threshold", title: qsTr("Threshold"), align: Text.AlignRight, text: r => r.threshold ?? "" },
            ] : []).concat([
                { key: "raw", title: qsTr("Raw value"), align: Text.AlignRight, text: r => r.raw },
                { key: "state", title: qsTr("State"), text: r => dialog.stateText[r.state] ?? r.state },
            ])
        }
    }
}
