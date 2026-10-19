import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Crashed processes from the journal, folded under their application.
ColumnLayout {
    id: page

    required property var monitor
    property bool active: false
    // When this window opened and when the machine booted (ms since epoch).
    property double openedAt: 0
    property double bootedAt: 0
    // 0: since opening, 1: since boot, 2: 7 days, 3: 30 days, 4: all.
    property int period: 2

    signal journalRequested(string title, var query)

    readonly property real unit: Kirigami.Units.gridUnit
    readonly property var result: monitor.crashes !== "" ? JSON.parse(monitor.crashes) : null
    readonly property var crashes: {
        const q = search.text.trim().toLowerCase()
        return (result?.crashes ?? []).map((c, i) => Object.assign(c, { id: c.time_us + ":" + i }))
            .filter(c => q === "" || [c.app, c.process, c.exe ?? "", c.signal ?? "", c.unit ?? ""]
                .some(x => x.toLowerCase().includes(q)))
    }
    readonly property var selected: table.selectedRow && table.selectedRow._group === undefined ? table.selectedRow : null

    spacing: 0

    function focusSearch() {
        search.forceActiveFocus()
        search.selectAll()
    }

    function sinceSecs() {
        const day = 86400
        switch (period) {
        case 0: return Math.floor(openedAt / 1000)
        case 1: return Math.floor(bootedAt / 1000)
        case 2: return Math.floor(Date.now() / 1000) - 7 * day
        case 3: return Math.floor(Date.now() / 1000) - 30 * day
        }
        return 0
    }

    function refresh() {
        monitor.refreshCrashes(sinceSecs())
    }

    function when(us) {
        return Qt.formatDateTime(new Date(us / 1000), "dd/MM/yyyy HH:mm:ss")
    }

    function kindName(c) {
        switch (c.kind) {
        case "oom": return qsTr("Out of memory")
        case "failed": return qsTr("Error exit")
        }
        return qsTr("Crash")
    }

    function cause(c) {
        if (c.kind === "oom") {
            return qsTr("Killed by the kernel: the system ran out of memory")
        }
        if (c.kind === "failed") {
            return qsTr("Exited with error code %1").arg(c.exit_status)
        }
        const why = {
            "SIGSEGV": qsTr("invalid memory access"),
            "SIGABRT": qsTr("the program stopped itself after detecting an error"),
            "SIGILL": qsTr("illegal instruction"),
            "SIGFPE": qsTr("arithmetic error"),
            "SIGBUS": qsTr("bus error"),
            "SIGTRAP": qsTr("breakpoint or failed assertion"),
            "SIGSYS": qsTr("forbidden system call"),
            "SIGQUIT": qsTr("quit with a core dump"),
        }[c.signal]
        return c.signal + (why ? " : " + why : "")
    }

    function journalAround(c) {
        const q = { since_us: c.time_us - 120 * 1000000, until_us: c.time_us + 10 * 1000000, limit: 1000 }
        if (c.unit && !/^user@\d+\.service$/.test(c.unit)) {
            q.unit = c.unit
            q.user = c.user_unit
        }
        page.journalRequested(qsTr("Journal around the crash of %1").arg(c.process), q)
    }

    function describe(c) {
        return [
            c.app + " - " + c.process,
            page.when(c.time_us) + "  " + page.cause(c),
            c.exe ? qsTr("Executable: %1").arg(c.exe) : "",
            c.command ? qsTr("Command: %1").arg(c.command) : "",
            c.unit ? qsTr("Unit: %1").arg(c.unit) : "",
            c.trace,
        ].filter(x => x !== "").join("\n")
    }

    onActiveChanged: if (active) refresh()
    onPeriodChanged: refresh()

    Timer {
        interval: 15000
        repeat: true
        running: page.active
        onTriggered: page.refresh()
    }

    PageHeader {
        title: qsTr("Crashes")

        QQC2.ComboBox {
            model: [qsTr("Since this window opened"), qsTr("Since the computer started"), qsTr("Last 7 days"),
                    qsTr("Last 30 days"), qsTr("Everything kept")]
            currentIndex: page.period
            implicitContentWidthPolicy: QQC2.ComboBox.WidestText
            onActivated: (i) => page.period = i
        }
        Kirigami.SearchField {
            id: search
            placeholderText: qsTr("Search an app or process")
            Layout.fillWidth: true
            Layout.preferredWidth: page.unit * 12
            Layout.maximumWidth: page.unit * 12
            Layout.minimumWidth: page.unit * 6
        }
    }

    Kirigami.InlineMessage {
        visible: (page.result?.error ?? "") !== ""
        text: Tr.core(page.result?.error ?? "")
        type: Kirigami.MessageType.Warning
        Layout.fillWidth: true
        Layout.leftMargin: page.unit
        Layout.rightMargin: page.unit
        Layout.bottomMargin: Kirigami.Units.smallSpacing
    }

    DataTable {
        id: table
        rows: page.crashes
        keyOf: c => c.id
        groupBy: c => c.app_key
        groupRow: (key, list) => ({
            app: list[0].app, process: "", count: list.length, time_us: list[0].time_us,
            kind: "", signal: "", exit_status: null, user: list[0].user,
        })
        expandAll: search.text.trim() !== ""
        sortColumn: "time"
        sortDescending: true
        placeholder: page.result === null ? "" : page.period === 0
            ? qsTr("No crash since this window opened") : qsTr("No crash in this period")
        Layout.fillWidth: true
        Layout.fillHeight: true
        columns: [
            { key: "name", title: qsTr("Application"), maxWidth: page.unit * 18,
              text: c => c._group !== undefined ? qsTr("%1 (%2)").arg(c.app).arg(c.count)
                       : c._child ? c.process : c.app + (c.process !== c.app ? " - " + c.process : "") },
            { key: "time", title: qsTr("Date"), text: c => page.when(c.time_us), value: c => c.time_us },
            { key: "kind", title: qsTr("Type"), text: c => c._group !== undefined ? "" : page.kindName(c) },
            { key: "cause", title: qsTr("Cause"), width: 0, text: c => c._group !== undefined ? "" : page.cause(c) },
            { key: "user", title: qsTr("User"), text: c => c.user },
        ]
        onActivated: (c) => {
            if (c._group !== undefined) {
                table.toggle(c._group)
            } else {
                page.journalAround(c)
            }
        }
        onContextMenuRequested: (row, item, position) => {
            menu.row = row
            menu.popup(item, position)
        }
    }

    Kirigami.Separator {
        visible: page.selected !== null
        Layout.fillWidth: true
    }

    // Details of the selected crash.
    ColumnLayout {
        visible: page.selected !== null
        spacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true
        Layout.preferredHeight: page.height * 0.4
        Layout.maximumHeight: page.height * 0.4
        Layout.margins: page.unit

        RowLayout {
            Kirigami.Heading {
                level: 3
                text: page.selected ? page.selected.app + " - " + page.selected.process : ""
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            QQC2.Button {
                text: qsTr("Journal")
                icon.source: Icons.of("journal", Kirigami.Theme.textColor)
                onClicked: page.journalAround(page.selected)
            }
            QQC2.Button {
                text: qsTr("Copy")
                icon.source: Icons.of("copy", Kirigami.Theme.textColor)
                onClicked: {
                    clipboard.text = page.describe(page.selected)
                    clipboard.selectAll()
                    clipboard.copy()
                }
            }
        }
        QQC2.Label {
            text: page.selected ? page.when(page.selected.time_us) + "  " + page.cause(page.selected) : ""
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        QQC2.Label {
            visible: !!page.selected?.exe
            text: qsTr("Executable: %1").arg(page.selected?.exe ?? "")
            opacity: 0.7
            elide: Text.ElideMiddle
            Layout.fillWidth: true
        }
        QQC2.Label {
            visible: !!page.selected?.unit
            text: qsTr("Unit: %1").arg(page.selected?.unit ?? "")
            opacity: 0.7
            elide: Text.ElideMiddle
            Layout.fillWidth: true
        }
        ThinScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            QQC2.TextArea {
                readOnly: true
                font: Kirigami.Theme.fixedWidthFont
                wrapMode: TextEdit.NoWrap
                text: page.selected?.trace || (page.selected?.kind === "crash"
                    ? qsTr("No stack trace: systemd-coredump did not record one for this crash.") : "")
            }
        }
    }

    QQC2.Menu {
        id: menu
        property var row: null
        readonly property bool single: row !== null && row._group === undefined

        QQC2.MenuItem {
            text: qsTr("Journal around the crash")
            icon.source: Icons.of("journal", Kirigami.Theme.textColor)
            enabled: menu.single
            onTriggered: page.journalAround(menu.row)
        }
        QQC2.MenuItem {
            text: qsTr("Copy details")
            icon.source: Icons.of("copy", Kirigami.Theme.textColor)
            enabled: menu.single
            onTriggered: {
                clipboard.text = page.describe(menu.row)
                clipboard.selectAll()
                clipboard.copy()
            }
        }
        QQC2.MenuSeparator {}
        QQC2.MenuItem {
            visible: menu.row?._group !== undefined
            height: visible ? implicitHeight : 0
            enabled: !table.expandAll
            text: table.isOpen(menu.row?._group) ? qsTr("Collapse") : qsTr("Expand")
            icon.source: Icons.of("collapse", Kirigami.Theme.textColor)
            onTriggered: table.toggle(menu.row._group)
        }
    }

    TextEdit {
        id: clipboard
        visible: false
    }
}
