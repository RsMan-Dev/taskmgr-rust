import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Context menu of a process or application row.
QQC2.Menu {
    id: menu

    property var entry: null
    property bool expanded: false
    property var processDetails: ({})

    signal signalRequested(var entry, string signal)
    signal toggleRequested(string groupKey)
    signal journalRequested(string title, var query)

        QQC2.MenuItem {
            text: qsTr("End task")
            icon.source: Icons.of("end-task", Kirigami.Theme.textColor)
            onTriggered: menu.signalRequested(menu.entry, "term")
        }
        QQC2.MenuItem {
            text: qsTr("Kill task")
            icon.source: Icons.of("kill", Kirigami.Theme.textColor)
            onTriggered: menu.signalRequested(menu.entry, "kill")
        }
        SignalMenu {
            onChosen: (sig) => menu.signalRequested(menu.entry, sig)
        }
        QQC2.MenuSeparator {}
        QQC2.MenuItem {
            visible: menu.entry?.type === "group" && menu.entry.group.processes.length > 1
            height: visible ? implicitHeight : 0
            text: menu.expanded ? qsTr("Collapse") : qsTr("Expand")
            icon.source: Icons.of("collapse", Kirigami.Theme.textColor)
            onTriggered: menu.toggleRequested(menu.entry.group.key)
        }
        QQC2.MenuItem {
            text: qsTr("Journal")
            icon.source: Icons.of("journal", Kirigami.Theme.textColor)
            enabled: menu.entry?.type === "process" || menu.entry?.type === "group"
            onTriggered: {
                const list = menu.entry.type === "group" ? menu.entry.group.processes : [menu.entry.proc]
                const name = menu.entry.type === "group" ? Tr.core(menu.entry.group.name) : menu.entry.proc.name
                menu.journalRequested(name, { pids: list.map(p => p.pid), limit: 1000 })
            }
        }
        QQC2.MenuItem {
            text: qsTr("Copy command line")
            icon.source: Icons.of("copy", Kirigami.Theme.textColor)
            enabled: menu.entry?.type === "process" || menu.entry?.group?.processes.length === 1
            onTriggered: {
                const p = menu.entry.type === "process" ? menu.entry.proc : menu.entry.group.processes[0]
                clipboard.text = menu.processDetails[p.pid]?.command ?? ""
                clipboard.selectAll()
                clipboard.copy()
            }
        }

    TextEdit {
        id: clipboard
        visible: false
    }
}
