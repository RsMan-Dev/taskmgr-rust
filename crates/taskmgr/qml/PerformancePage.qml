import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: page

    required property var snapshot
    required property var monitor

    readonly property var colors: ({
        cpu: Kirigami.Theme.highlightColor,
        memory: "#9b59b6",
        disk: "#1d99f3",
        network: "#f67400",
        gpu: "#1abc9c",
        sensors: "#da4453",
        temperature: "#da4453",
        clock: "#f67400",
        voltage: "#fdbc4b",
        power: "#27ae60",
        efficiency: "#1abc9c",
        fan: "#3daee9",
    })

    property string selectedKey: "cpu"
    // Entries hidden from the list; all are shown while rearranging.
    readonly property var hidden: Prefs.hiddenResources.split(",").filter(k => k !== "")
    property bool showHidden: false

    function moveResource(key, delta) {
        const keys = keyList.split(",")
        const i = keys.indexOf(key)
        const j = i + delta
        if (i < 0 || j < 0 || j >= keys.length) {
            return
        }
        keys.splice(i, 1)
        keys.splice(j, 0, key)
        Prefs.resourceOrder = keys.join(",")
    }

    function toggleHidden(key) {
        Prefs.hiddenResources = (hidden.includes(key) ? hidden.filter(k => k !== key) : hidden.concat([key])).join(",")
    }
    // CPU graph: "overall", "logical", "clocks" (MHz per processor) or "numa".
    property string cpuView: "overall"

    ComputeResources {
        id: compute
        monitor: page.monitor
        colors: page.colors
        cpuView: page.cpuView
    }
    DeviceResources {
        id: devices
        monitor: page.monitor
        colors: page.colors
    }


    // Changes only when a device comes or goes, so delegates and selection
    // survive updates.
    readonly property string keyList: {
        const s = snapshot
        if (!s) {
            return ""
        }
        const all = ["cpu", "memory"]
            .concat(s.disks.map(d => "disk:" + d.name))
            .concat(s.networks.map(n => "net:" + n.name))
            .concat(s.gpus.map(g => "gpu:" + g.pci_slot))
            .concat(s.batteries.map(b => "battery:" + b.name))
            .concat(s.fans.map(f => "fan:" + f.key))
            .concat(["consumption"])
            .concat(s.sensors.length > 0 ? ["sensors"] : [])
        // The user's order first, new devices after in their natural place.
        const order = Prefs.resourceOrder.split(",")
        const rank = k => {
            const i = order.indexOf(k)
            return i < 0 ? order.length + all.indexOf(k) : i
        }
        return all.filter(k => showHidden || !hidden.includes(k)).sort((a, b) => rank(a) - rank(b)).join(",")
    }

    readonly property var selected: snapshot ? resource(selectedKey) ?? resource("cpu") : null

    function resource(key) {
        const [kind, id] = [key.split(":")[0], key.slice(key.indexOf(":") + 1)]
        const s = snapshot
        switch (kind) {
        case "cpu": return compute.cpu(s)
        case "consumption": return compute.consumption(s)
        case "battery": return compute.battery(s, id)
        case "memory": return compute.memory(s)
        case "disk": return devices.disk(s, id)
        case "net": return devices.net(s, id)
        case "gpu": return devices.gpu(s, id)
        case "fan": return devices.fan(s, id)
        case "sensors": return devices.sensors(s)
        }
        return null
    }

    function detailsText(r) {
        return [r.title + "  " + r.header]
            .concat((r.stats ?? []).map(x => x[0] + " : " + x[1]))
            .concat((r.details ?? []).map(x => x[0] + " : " + x[1]))
            .join("\n")
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        PageHeader {
            title: qsTr("Performance")
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            ThinScrollView {
                Layout.preferredWidth: Math.min(Kirigami.Units.gridUnit * 16, Math.max(Kirigami.Units.gridUnit * 11, page.width * 0.32))
                Layout.fillWidth: false
                Layout.fillHeight: true
                QQC2.ScrollBar.horizontal.policy: QQC2.ScrollBar.AlwaysOff

                ListView {
                    id: list
                    model: page.keyList === "" ? [] : page.keyList.split(",")
                    spacing: Kirigami.Units.smallSpacing
                    topMargin: Kirigami.Units.largeSpacing
                    leftMargin: Kirigami.Units.largeSpacing
                    rightMargin: Kirigami.Units.largeSpacing
                    bottomMargin: Kirigami.Units.largeSpacing
                    clip: true
                    // Selection follows the key: the view resets its index
                    // whenever a device comes or goes.
                    currentIndex: -1
                    activeFocusOnTab: true

                    function step(delta) {
                        const keys = model
                        const i = Math.max(0, keys.indexOf(page.selectedKey)) + delta
                        if (i >= 0 && i < keys.length) {
                            page.selectedKey = keys[i]
                            positionViewAtIndex(i, ListView.Contain)
                        }
                    }
                    Keys.onUpPressed: step(-1)
                    Keys.onDownPressed: step(1)

                    delegate: ResourceCard {
                        required property string modelData
                        width: ListView.view.width - ListView.view.leftMargin - ListView.view.rightMargin
                        resource: page.resource(modelData) ?? { title: "", subtitle: "", summary: "", series: [], maximum: 100, color: "gray" }
                        selected: modelData === page.selectedKey
                        opacity: page.hidden.includes(modelData) ? 0.45 : 1
                        onMenuRequested: {
                            resourceMenu.key = modelData
                            resourceMenu.popup()
                        }
                        onClicked: {
                            page.selectedKey = modelData
                            ListView.view.forceActiveFocus()
                        }
                    }
                }
            }

            Kirigami.Separator {
                Layout.fillHeight: true
            }

            ThinScrollView {
                id: detailScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                // Scrolls sideways instead of clipping once the content
                // reaches its minimum width.
                contentWidth: Math.max(availableWidth, Kirigami.Units.gridUnit * 22)

                ResourceDetail {
                    r: page.selected
                    snapshot: page.snapshot
                    monitor: page.monitor
                    colors: page.colors
                    profileNames: compute.profileNames
                    pageHeight: page.height
                    width: detailScroll.contentWidth
                    onChartMenuRequested: chartMenu.popup()
                    onActionRequested: (a) => a.id.startsWith("smart:")
                        ? smartDialog.show(a.id.slice(6), a.label) : ejectDialog.eject(a.id.slice(6), a.label)
                }
            }
        }
    }

    QQC2.Menu {
        id: resourceMenu
        property string key: ""

        QQC2.MenuItem {
            text: page.hidden.includes(resourceMenu.key) ? qsTr("Show in the list") : qsTr("Hide from the list")
            icon.source: Icons.of("hide", Kirigami.Theme.textColor)
            enabled: resourceMenu.key !== "cpu"
            onTriggered: page.toggleHidden(resourceMenu.key)
        }
        QQC2.MenuItem {
            text: qsTr("Move up")
            icon.source: Icons.of("sort-up", Kirigami.Theme.textColor)
            onTriggered: page.moveResource(resourceMenu.key, -1)
        }
        QQC2.MenuItem {
            text: qsTr("Move down")
            icon.source: Icons.of("sort-down", Kirigami.Theme.textColor)
            onTriggered: page.moveResource(resourceMenu.key, 1)
        }
        QQC2.MenuSeparator {}
        QQC2.MenuItem {
            text: qsTr("Show hidden entries")
            checkable: true
            checked: page.showHidden
            enabled: page.hidden.length > 0 || page.showHidden
            onTriggered: page.showHidden = !page.showHidden
        }
        QQC2.MenuItem {
            text: qsTr("Reset the list")
            onTriggered: {
                Prefs.resourceOrder = ""
                Prefs.hiddenResources = ""
            }
        }
    }

    SmartDialog {
        id: smartDialog
        monitor: page.monitor
    }

    EjectDialog {
        id: ejectDialog
        monitor: page.monitor
    }

    QQC2.Menu {
        id: chartMenu

        QQC2.MenuItem {
            visible: !!page.selected?.cpu
            height: visible ? implicitHeight : 0
            text: qsTr("Overall utilization")
            checkable: true
            checked: page.cpuView === "overall"
            onTriggered: page.cpuView = "overall"
        }
        QQC2.MenuItem {
            visible: !!page.selected?.cpu
            height: visible ? implicitHeight : 0
            text: qsTr("Logical processors")
            checkable: true
            checked: page.cpuView === "logical"
            onTriggered: page.cpuView = "logical"
        }
        QQC2.MenuItem {
            visible: !!page.selected?.hasClocks
            height: visible ? implicitHeight : 0
            text: qsTr("Frequency per processor")
            checkable: true
            checked: page.cpuView === "clocks"
            onTriggered: page.cpuView = "clocks"
        }
        QQC2.MenuItem {
            visible: !!page.selected?.hasNuma
            height: visible ? implicitHeight : 0
            text: qsTr("NUMA nodes")
            checkable: true
            checked: page.cpuView === "numa"
            onTriggered: page.cpuView = "numa"
        }
        QQC2.MenuItem {
            visible: !!page.selected?.cpu
            height: visible ? implicitHeight : 0
            text: qsTr("Show kernel times")
            checkable: true
            checked: Prefs.kernelTimes
            onTriggered: Prefs.kernelTimes = !Prefs.kernelTimes
        }
        QQC2.MenuSeparator {
            visible: !!page.selected?.cpu
            height: visible ? implicitHeight : 0
        }
        QQC2.MenuItem {
            text: qsTr("Copy details")
            icon.source: Icons.of("copy", Kirigami.Theme.textColor)
            onTriggered: {
                clipboard.text = page.detailsText(page.selected)
                clipboard.selectAll()
                clipboard.copy()
            }
        }
    }

    TextEdit {
        id: clipboard
        visible: false
    }
}
