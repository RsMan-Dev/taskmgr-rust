import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

QQC2.ItemDelegate {
    id: row

    required property var entry
    property bool selected: false
    property bool expanded: false
    property var columns: []

    signal toggleRequested()
    signal menuRequested(point position)

    // entry turns null for an instant when its row leaves the list.
    readonly property bool isGroup: entry?.type === "group"
    readonly property var group: entry?.group ?? null
    readonly property var proc: entry?.proc ?? null
    readonly property bool expandable: isGroup && group.processes.length > 1
    readonly property var metrics: (isGroup ? group : proc) ?? {}
    readonly property bool secondary: !isGroup && !!proc && (proc.role ?? "normal") !== "normal"

    padding: 0
    hoverEnabled: true

    background: Rectangle {
        color: row.selected ? Qt.alpha(Kirigami.Theme.highlightColor, 0.22)
             : row.hovered ? Qt.alpha(Kirigami.Theme.highlightColor, 0.08) : "transparent"
    }

    onDoubleClicked: if (expandable) toggleRequested()

    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: (point) => row.menuRequested(point.position)
    }
    TapHandler {
        acceptedDevices: PointerDevice.TouchScreen
        onLongPressed: row.menuRequested(point.position)
    }

    contentItem: RowLayout {
        spacing: 0

        RowLayout {
            spacing: Kirigami.Units.smallSpacing
            clip: true
            Layout.fillWidth: true
            Layout.fillHeight: true
            // Without this the cell would never be narrower than its chips.
            Layout.minimumWidth: 0
            Layout.leftMargin: row.isGroup ? Kirigami.Units.smallSpacing : Kirigami.Units.gridUnit * 2.6
            Layout.rightMargin: Kirigami.Units.smallSpacing

            QQC2.ToolButton {
                visible: row.isGroup
                enabled: row.expandable
                opacity: row.expandable ? 1 : 0
                icon.source: Icons.of(row.expanded ? "collapse" : "expand", Kirigami.Theme.textColor)
                icon.width: Kirigami.Units.iconSizes.small
                icon.height: Kirigami.Units.iconSizes.small
                display: QQC2.AbstractButton.IconOnly
                flat: true
                text: row.expanded ? qsTr("Collapse") : qsTr("Expand")
                onClicked: row.toggleRequested()
                Layout.preferredWidth: Kirigami.Units.gridUnit * 1.4
                Layout.preferredHeight: Kirigami.Units.gridUnit * 1.4
            }

            Kirigami.Icon {
                visible: row.isGroup
                readonly property string generic: Icons.of(row.isGroup ? row.group.kind : "app", Kirigami.Theme.textColor)
                source: row.isGroup && row.group.icon ? row.group.icon : generic
                fallback: generic
                Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
            }

            QQC2.Label {
                text: row.isGroup
                    ? Tr.core(row.group.name) + (row.expandable ? "  (" + row.group.processes.length + ")" : "")
                    : row.proc?.name ?? ""
                elide: Text.ElideRight
                opacity: row.secondary ? 0.6 : 1
                Layout.fillWidth: true
                // Text widths are fractional: without slack, pixel rounding elides.
                Layout.maximumWidth: Math.ceil(implicitWidth) + 1
                Layout.minimumWidth: Kirigami.Units.gridUnit * 4
            }

            QQC2.Label {
                readonly property var titles: row.isGroup ? (row.expandable ? [] : row.group.windows ?? []) : (row.proc?.windows ?? [])
                visible: titles.length > 0
                text: titles.length > 0 ? titles[0] : ""
                opacity: 0.6
                elide: Text.ElideRight
                Layout.fillWidth: true
                Layout.maximumWidth: Math.ceil(implicitWidth) + 1
                Layout.minimumWidth: Math.min(implicitWidth, Kirigami.Units.gridUnit * 4)
            }

            Repeater {
                model: row.isGroup ? row.group.via ?? [] : []
                Chip {
                    required property string modelData
                    text: modelData
                }
            }

            Chip {
                visible: row.secondary
                text: row.secondary && row.proc.role === "wrapper" ? qsTr("wrapper") : "Wine"
            }

            QQC2.Label {
                visible: !row.isGroup || !row.expandable
                text: qsTr("PID %1").arg(row.isGroup ? row.group.processes[0].pid : row.proc?.pid ?? "")
                opacity: 0.5
                font: Kirigami.Theme.smallFont
                Layout.minimumWidth: implicitWidth
            }

            Item {
                Layout.fillWidth: true
            }
        }

        Repeater {
            model: row.columns
            HeatCell {
                required property var modelData
                readonly property real v: modelData.value(row.metrics)
                value: v
                reference: modelData.reference
                text: modelData.text(v)
                zero: v <= 0
                emphasized: row.isGroup
                Layout.preferredWidth: modelData.width
                Layout.fillHeight: true
            }
        }
    }

    component Chip: Rectangle {
        property alias text: chipLabel.text
        // The name is elided first; chips keep their size.
        Layout.minimumWidth: implicitWidth
        radius: height / 2
        color: Qt.alpha(Kirigami.Theme.textColor, 0.08)
        implicitWidth: chipLabel.implicitWidth + Kirigami.Units.largeSpacing * 2
        implicitHeight: chipLabel.implicitHeight + 2
        QQC2.Label {
            id: chipLabel
            anchors.centerIn: parent
            font: Kirigami.Theme.smallFont
            opacity: 0.8
        }
    }
}
