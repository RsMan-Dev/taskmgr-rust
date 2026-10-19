import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: view

    property var groups: []
    property var energy: null

    readonly property var icons: ({
        cpu: "cpu",
        gpu: "video-display",
        disk: "drive-harddisk",
        board: "computer",
        network: "network-wired",
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

    Kirigami.InlineMessage {
        visible: view.energy?.status === "denied"
        type: Kirigami.MessageType.Information
        text: "Consommation du processeur indisponible : le noyau réserve le compteur RAPL à root. "
            + "La règle udev qui l'ouvre, et son compromis de sécurité, sont décrits dans docs/energy.md."
        Layout.fillWidth: true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 48
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
                        source: view.icons[card.modelData.kind] ?? "dialog-information"
                        Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                        Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                    }
                    Kirigami.Heading {
                        text: card.modelData.name
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
                            text: sensorRow.modelData.label
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
                            text: sensorRow.modelData.critical !== null ? "crit. " + Format.temperature(sensorRow.modelData.critical) : ""
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
