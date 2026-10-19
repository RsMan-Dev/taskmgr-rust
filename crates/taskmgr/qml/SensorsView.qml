import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: view

    property var groups: []

    readonly property var icons: ({
        cpu: "cpu",
        gpu: "gpu",
        disk: "disk",
        board: "board",
        network: "network",
    })

    spacing: Kirigami.Units.largeSpacing

    function tone(s) {
        if (s.value >= (s.critical ?? 1000)) {
            return Kirigami.Theme.negativeTextColor
        }
        if (s.value >= (s.high ?? 90)) {
            return Kirigami.Theme.neutralTextColor
        }
        return Kirigami.Theme.textColor
    }

    Repeater {
        model: view.groups

        Card {
            id: card
            required property var modelData
            implicitHeight: content.implicitHeight + Kirigami.Units.gridUnit * 1.5

            ColumnLayout {
                id: content
                spacing: Kirigami.Units.smallSpacing
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: Kirigami.Units.gridUnit
                anchors.topMargin: Kirigami.Units.largeSpacing * 1.5

                RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    Layout.bottomMargin: Kirigami.Units.smallSpacing
                    Kirigami.Icon {
                        source: Icons.of(view.icons[card.modelData.kind] ?? "sensor")
                        isMask: true
                        color: Kirigami.Theme.textColor
                        Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                        Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                    }
                    Kirigami.Heading {
                        text: Tr.core(card.modelData.name)
                        level: 4
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }

                Repeater {
                    model: card.modelData.sensors

                    RowLayout {
                        id: sensorRow
                        required property var modelData
                        spacing: Kirigami.Units.largeSpacing
                        Layout.fillWidth: true

                        QQC2.Label {
                            text: Tr.core(sensorRow.modelData.label)
                            opacity: 0.7
                            elide: Text.ElideRight
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 9
                        }
                        UsageChart {
                            compact: true
                            maximum: 110
                            accent: view.tone(sensorRow.modelData) === Kirigami.Theme.textColor
                                ? Kirigami.Theme.highlightColor : view.tone(sensorRow.modelData)
                            series: [{ values: sensorRow.modelData.history, color: accent }]
                            Layout.fillWidth: true
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 1.4
                        }
                        QQC2.Label {
                            text: Format.temperature(sensorRow.modelData.value)
                            color: view.tone(sensorRow.modelData)
                            font.weight: Font.DemiBold
                            font.features: { "tnum": 1 }
                            horizontalAlignment: Text.AlignRight
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 3.5
                        }
                        QQC2.Label {
                            text: sensorRow.modelData.critical !== null ? qsTr("crit. %1").arg(Format.temperature(sensorRow.modelData.critical)) : ""
                            opacity: 0.5
                            font: Kirigami.Theme.smallFont
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 4.5
                        }
                    }
                }
            }
        }
    }
}
