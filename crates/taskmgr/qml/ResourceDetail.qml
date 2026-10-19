import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Right-hand side of the Performance page for one resource.
ColumnLayout {
    id: detail

    property var r: null
    property var snapshot: null
    property var monitor: null
    property var colors: ({})
    property var profileNames: ({})
    property real pageHeight: 600
    readonly property real inner: width - Kirigami.Units.gridUnit * 2

    signal chartMenuRequested()

    visible: r !== null
    spacing: Kirigami.Units.largeSpacing

    Item {
        Layout.preferredHeight: Kirigami.Units.smallSpacing
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing
        Layout.fillWidth: true
        Layout.leftMargin: Kirigami.Units.gridUnit
        Layout.rightMargin: Kirigami.Units.gridUnit
        Layout.bottomMargin: Kirigami.Units.gridUnit

        RowLayout {
            Kirigami.Heading {
                text: detail.r?.title ?? ""
                level: 1
            }
            QQC2.Label {
                text: detail.r?.header ?? ""
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.2
                horizontalAlignment: Text.AlignRight
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        EnergyNotice {
            visible: (!!detail.r?.energy || !!detail.r?.sensors) && detail.snapshot?.energy?.status === "denied"
            monitor: detail.monitor
            energy: detail.snapshot?.energy ?? null
            Layout.fillWidth: true
        }

        RowLayout {
            visible: !!detail.r?.profile
            spacing: Kirigami.Units.largeSpacing
            QQC2.Label {
                text: qsTr("Power profile")
            }
            QQC2.ComboBox {
                readonly property var profiles: detail.r?.profile?.profiles ?? []
                model: profiles.map(p => detail.profileNames[p] ?? p)
                implicitContentWidthPolicy: QQC2.ComboBox.WidestText
                currentIndex: profiles.indexOf(detail.r?.profile?.profile ?? "")
                onActivated: (i) => detail.monitor.setPowerProfile(profiles[i])
            }
            QQC2.Label {
                visible: (detail.r?.profile?.degraded ?? "") !== ""
                text: qsTr("Performance held back: %1").arg(detail.r?.profile?.degraded ?? "")
                opacity: 0.7
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        SensorsView {
            visible: !!detail.r?.sensors
            groups: detail.r?.sensors ? detail.snapshot?.sensors ?? [] : []
            Layout.fillWidth: true
        }

        ChartBlock {
            id: mainChart
            visible: !detail.r?.sensors && !(detail.r?.grid && detail.r?.gridReplacesChart)
            chart: detail.r
            accent: detail.r?.color ?? "gray"
            chartHeight: Math.max(Kirigami.Units.gridUnit * 7, detail.pageHeight * (detail.r?.secondChart || detail.r?.charts ? 0.3 : 0.42))
            Layout.fillWidth: true
            onContextMenuRequested: (position) => detail.chartMenuRequested()
        }

        MemoryComposition {
            visible: !!detail.r?.composition
            composition: detail.r?.composition ?? null
            accent: detail.colors.memory
            Layout.fillWidth: true
        }

        ProcessorGrid {
            grid: detail.r?.grid ?? null
            colors: detail.colors
            cellHeight: detail.pageHeight * (detail.r?.gridReplacesChart ? 0.42 : 0.25)
            Layout.fillWidth: true
            onMenuRequested: if (detail.r?.cpu) detail.chartMenuRequested()
        }

        ChartBlock {
            visible: !!detail.r?.secondChart
            chart: detail.r?.secondChart ?? null
            accent: detail.r?.color ?? "gray"
            chartHeight: Math.max(Kirigami.Units.gridUnit * 5, detail.pageHeight * 0.18)
            Layout.fillWidth: true
        }

        GridLayout {
            visible: (detail.r?.charts?.length ?? 0) > 0
            columns: Math.max(1, Math.min(detail.r?.charts?.length ?? 1, Math.floor(detail.inner / (Kirigami.Units.gridUnit * 11))))
            columnSpacing: Kirigami.Units.gridUnit
            rowSpacing: Kirigami.Units.largeSpacing
            Layout.fillWidth: true

            Repeater {
                model: detail.r?.charts ?? []
                MiniChart {
                    required property var modelData
                    note: modelData.note ?? ""
                    actionText: modelData.actionText ?? ""
                    onActionTriggered: detail.monitor.enableNetworkMonitor()
                    label: modelData.label
                    value: modelData.value
                    values: modelData.values
                    maximum: modelData.maximum
                    accent: modelData.accent
                    unavailable: modelData.unavailable
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                }
            }
        }

        // Live values and static details side by side, stacked when narrow.
        GridLayout {
            visible: !detail.r?.sensors
            columns: detail.width > Kirigami.Units.gridUnit * 34 ? 2 : 1
            columnSpacing: Kirigami.Units.gridUnit * 2
            rowSpacing: Kirigami.Units.gridUnit
            Layout.fillWidth: true

            GridLayout {
                columns: detail.inner > Kirigami.Units.gridUnit * 24 ? 2 : 1
                columnSpacing: Kirigami.Units.gridUnit * 2
                rowSpacing: Kirigami.Units.largeSpacing
                Layout.alignment: Qt.AlignTop

                Repeater {
                    model: detail.r?.stats ?? []
                    Stat {
                        required property var modelData
                        label: modelData[0]
                        value: modelData[1]
                    }
                }
            }

            ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.alignment: Qt.AlignTop
                Layout.fillWidth: true

                Repeater {
                    model: detail.r?.details ?? []
                    DetailRow {
                        required property var modelData
                        label: modelData[0]
                        value: String(modelData[1])
                        Layout.fillWidth: true
                    }
                }
            }
        }

        MemoryModules {
            visible: (detail.r?.dimms?.length ?? 0) > 0
            dimms: detail.r?.dimms ?? []
            memory: detail.snapshot?.memory ?? null
            accent: detail.colors.memory
            Layout.fillWidth: true
        }
    }
}
