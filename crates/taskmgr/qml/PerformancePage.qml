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
    })

    property string selectedKey: "cpu"
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

    // The profile can change from the desktop's own controls.
    Timer {
        interval: 10000
        repeat: true
        triggeredOnStart: true
        running: page.visible
        onTriggered: page.monitor.refreshPower()
    }

    // Changes only when a device comes or goes, so delegates and selection
    // survive updates.
    readonly property string keyList: {
        const s = snapshot
        if (!s) {
            return ""
        }
        return ["cpu", "memory"]
            .concat(s.disks.map(d => "disk:" + d.name))
            .concat(s.networks.map(n => "net:" + n.name))
            .concat(s.gpus.map(g => "gpu:" + g.pci_slot))
            .concat(s.batteries.map(b => "battery:" + b.name))
            .concat(["power"])
            .concat(s.sensors.length > 0 ? ["sensors"] : [])
            .join(",")
    }

    readonly property var selected: snapshot ? resource(selectedKey) ?? resource("cpu") : null

    function resource(key) {
        const [kind, id] = [key.split(":")[0], key.slice(key.indexOf(":") + 1)]
        const s = snapshot
        switch (kind) {
        case "cpu": return compute.cpu(s)
        case "power": return compute.powerAndClocks(s)
        case "battery": return compute.battery(s, id)
        case "memory": return compute.memory(s)
        case "disk": return devices.disk(s, id)
        case "net": return devices.net(s, id)
        case "gpu": return devices.gpu(s, id)
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
                }
            }
        }
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
