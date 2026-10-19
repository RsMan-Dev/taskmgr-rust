import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

QQC2.ScrollView {
    id: page

    required property var info
    property var snapshot

    contentWidth: availableWidth

    // [title, icon, [[label, value], ...]]
    readonly property var sections: {
        const i = info
        if (!i) {
            return []
        }
        const s = snapshot
        const clean = rows => rows.filter(r => r[1] !== null && r[1] !== undefined && r[1] !== "")
        const cpu = i.cpu
        return [
            ["Système", "preferences-system-linux", clean([
                ["Nom de l'appareil", i.hostname],
                ["Système d'exploitation", i.os],
                ["Noyau", i.kernel ? "Linux " + i.kernel : null],
                ["Architecture", i.architecture],
                ["Bureau", i.desktop ? i.desktop + (i.session ? " (" + i.session + ")" : "") : null],
            ])],
            ["Carte mère", "computer", clean([
                ["Fabricant", i.vendor],
                ["Produit", i.manufacturer !== i.board ? i.manufacturer : null],
                ["Modèle", i.board],
                ["BIOS", i.bios],
                ["Micrologiciel", i.firmware],
            ])],
            ["Processeur et mémoire", "cpu", clean([
                ["Processeur", cpu.model],
                ["Cœurs", cpu.cores + " cœurs, " + cpu.logical + " processeurs logiques"],
                ["Mémoire utilisable", Format.bytes(i.memory_total)],
            ])],
            ["Cartes graphiques", "video-display", s ? s.gpus.map(g => [
                "GPU " + g.index,
                g.name + " (pilote " + g.driver + (g.driver_version ? " " + g.driver_version : "") + ")",
            ]) : []],
            ["Disques", "drive-harddisk", s ? s.disks.map(d => [
                "Disque " + d.index,
                d.model + ", " + Format.bytes(d.capacity) + ", " + d.kind,
            ]) : []],
            ["Réseau", "network-wired", s ? s.networks.map(n => [n.kind + " (" + n.name + ")", n.adapter]) : []],
        ].filter(sec => sec[2].length > 0)
    }

    function asText() {
        return sections.map(sec => sec[0] + "\n" + sec[2].map(r => "  " + r[0] + " : " + r[1]).join("\n")).join("\n\n")
    }

    ColumnLayout {
        width: page.availableWidth
        spacing: Kirigami.Units.gridUnit

        PageHeader {
            title: "Système"

            QQC2.Button {
                text: copied.running ? "Copié" : "Copier"
                icon.name: copied.running ? "checkmark" : "edit-copy"
                onClicked: {
                    clipboardHelper.text = page.asText()
                    clipboardHelper.selectAll()
                    clipboardHelper.copy()
                    copied.restart()
                }
                Timer {
                    id: copied
                    interval: 2000
                }
            }
        }

        TextEdit {
            id: clipboardHelper
            visible: false
        }

        Card {
            visible: page.info !== null
            Layout.leftMargin: Kirigami.Units.gridUnit
            Layout.rightMargin: Kirigami.Units.gridUnit

            RowLayout {
                spacing: Kirigami.Units.gridUnit
                anchors.fill: parent
                anchors.margins: Kirigami.Units.gridUnit

                Kirigami.Icon {
                    source: "computer"
                    Layout.preferredWidth: Kirigami.Units.iconSizes.huge
                    Layout.preferredHeight: Kirigami.Units.iconSizes.huge
                }
                ColumnLayout {
                    spacing: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true
                    Kirigami.Heading {
                        text: page.info?.hostname ?? ""
                        level: 2
                    }
                    QQC2.Label {
                        text: page.info?.os ?? ""
                    }
                    QQC2.Label {
                        text: page.info ? [page.info.manufacturer, page.info.kernel ? "Linux " + page.info.kernel : null]
                            .filter(x => x).join("  ·  ") : ""
                        opacity: 0.7
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
            }
            implicitHeight: Kirigami.Units.iconSizes.huge + Kirigami.Units.gridUnit * 2
        }

        Repeater {
            model: page.sections

            Card {
                id: card
                required property var modelData
                Layout.leftMargin: Kirigami.Units.gridUnit
                Layout.rightMargin: Kirigami.Units.gridUnit
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
                            source: card.modelData[1]
                            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                        }
                        Kirigami.Heading {
                            text: card.modelData[0]
                            level: 4
                        }
                    }
                    Repeater {
                        model: card.modelData[2]
                        DetailRow {
                            required property var modelData
                            label: modelData[0]
                            value: String(modelData[1])
                            labelWidth: Kirigami.Units.gridUnit * 11
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }

        Item {
            Layout.preferredHeight: Kirigami.Units.smallSpacing
        }
    }

    component Card: Rectangle {
        Layout.fillWidth: true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 48
        radius: Kirigami.Units.cornerRadius * 2
        color: Qt.alpha(Kirigami.Theme.textColor, 0.04)
        border.color: Qt.alpha(Kirigami.Theme.textColor, 0.09)
    }
}
