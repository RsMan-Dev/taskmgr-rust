import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// One card per memory module. The memory controller interleaves pages across
// channels, so each module holds a share of the used memory proportional to
// its size: the per-module figure is an estimate, said as such.
ColumnLayout {
    id: modules

    property var dimms: []
    property var memory: null
    property color accent: Kirigami.Theme.highlightColor

    spacing: Kirigami.Units.largeSpacing

    Kirigami.Heading {
        text: "Barrettes"
        level: 3
    }

    GridLayout {
        columns: Math.max(1, Math.min(modules.dimms.length, Math.floor(modules.width / (Kirigami.Units.gridUnit * 16))))
        columnSpacing: Kirigami.Units.largeSpacing
        rowSpacing: Kirigami.Units.largeSpacing
        Layout.fillWidth: true

        Repeater {
            model: modules.dimms

            Card {
                id: card
                required property var modelData
                readonly property real share: modules.memory ? modules.memory.used * modelData.size / modules.memory.total : 0
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                implicitHeight: content.implicitHeight + Kirigami.Units.gridUnit * 1.5

                ColumnLayout {
                    id: content
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: Kirigami.Units.gridUnit
                    anchors.topMargin: Kirigami.Units.largeSpacing * 1.5
                    spacing: Kirigami.Units.smallSpacing

                    RowLayout {
                        Kirigami.Icon {
                            source: "media-flash"
                            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                        }
                        Kirigami.Heading {
                            text: card.modelData.locator + (card.modelData.bank ? "  ·  " + card.modelData.bank : "")
                            level: 4
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    QQC2.Label {
                        text: [Format.bytes(card.modelData.size), card.modelData.kind,
                               card.modelData.speed_mts ? card.modelData.speed_mts + " MT/s" : null].filter(x => x).join("  ")
                    }
                    QQC2.Label {
                        visible: text !== ""
                        text: [card.modelData.manufacturer, card.modelData.part].filter(x => x).join("  ")
                        opacity: 0.7
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    Rectangle {
                        radius: height / 2
                        color: Qt.alpha(Kirigami.Theme.textColor, 0.1)
                        Layout.fillWidth: true
                        Layout.preferredHeight: Kirigami.Units.smallSpacing * 1.5
                        Layout.topMargin: Kirigami.Units.smallSpacing

                        Rectangle {
                            width: parent.width * Math.min(1, card.share / card.modelData.size)
                            height: parent.height
                            radius: parent.radius
                            color: modules.accent
                        }
                    }
                    QQC2.Label {
                        text: "\u2248 " + Format.bytes(card.share) + " utilisés"
                        font: Kirigami.Theme.smallFont
                        opacity: 0.7
                    }
                }
            }
        }
    }

    QQC2.Label {
        text: "Estimation : le contrôleur mémoire entrelace les données entre les barrettes, "
            + "chacune en porte une part proportionnelle à sa taille."
        wrapMode: Text.Wrap
        opacity: 0.6
        font: Kirigami.Theme.smallFont
        Layout.fillWidth: true
    }
}
