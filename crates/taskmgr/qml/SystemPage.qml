import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Summary of the machine, in the spirit of msinfo32.
QQC2.ScrollView {
    id: page

    required property var info
    property var snapshot

    contentWidth: availableWidth

    // [section, [[label, value], ...]], empty values dropped.
    readonly property var sections: {
        const i = info
        if (!i) {
            return []
        }
        const s = snapshot
        const clean = rows => rows.filter(r => r[1] !== null && r[1] !== undefined && r[1] !== "")
        const cpu = i.cpu
        return [
            ["Système", clean([
                ["Nom de l'appareil", i.hostname],
                ["Système d'exploitation", i.os],
                ["Noyau", i.kernel ? "Linux " + i.kernel : null],
                ["Architecture", i.architecture],
                ["Bureau", i.desktop ? i.desktop + (i.session ? " (" + i.session + ")" : "") : null],
            ])],
            ["Matériel", clean([
                ["Fabricant", i.manufacturer],
                ["Carte mère", i.board],
                ["BIOS", i.bios],
                ["Micrologiciel", i.firmware],
                ["Processeur", cpu.model],
                ["Cœurs", cpu.cores + " cœurs, " + cpu.logical + " processeurs logiques"],
                ["Mémoire utilisable", Format.bytes(i.memory_total)],
            ])],
            ["Cartes graphiques", s ? s.gpus.map(g => ["GPU " + g.index, g.name + (g.driver_version ? " (pilote " + g.driver + " " + g.driver_version + ")" : " (pilote " + g.driver + ")")]) : []],
            ["Disques", s ? s.disks.map(d => ["Disque " + d.index, d.model + ", " + Format.bytes(d.capacity) + ", " + d.kind]) : []],
            ["Réseau", s ? s.networks.map(n => [n.kind + " (" + n.name + ")", n.adapter]) : []],
        ].filter(sec => sec[1].length > 0)
    }

    function asText() {
        return sections.map(sec => sec[0] + "\n" + sec[1].map(r => "  " + r[0] + " : " + r[1]).join("\n")).join("\n\n")
    }

    ColumnLayout {
        width: page.availableWidth
        spacing: Kirigami.Units.largeSpacing

        RowLayout {
            Layout.margins: Kirigami.Units.gridUnit
            Layout.bottomMargin: 0
            Kirigami.Heading {
                text: "Système"
                level: 1
                Layout.fillWidth: true
            }
            QQC2.Button {
                text: "Copier"
                icon.name: "edit-copy"
                onClicked: {
                    clipboardHelper.text = page.asText()
                    clipboardHelper.selectAll()
                    clipboardHelper.copy()
                }
            }
        }

        // QML has no clipboard API; a hidden text field does the copy.
        TextEdit {
            id: clipboardHelper
            visible: false
        }

        Repeater {
            model: page.sections

            ColumnLayout {
                required property var modelData
                spacing: Kirigami.Units.smallSpacing
                Layout.leftMargin: Kirigami.Units.gridUnit
                Layout.rightMargin: Kirigami.Units.gridUnit
                Layout.topMargin: Kirigami.Units.largeSpacing
                Layout.maximumWidth: Kirigami.Units.gridUnit * 40

                Kirigami.Heading {
                    text: modelData[0]
                    level: 3
                }
                Repeater {
                    model: modelData[1]
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

        Item { Layout.preferredHeight: Kirigami.Units.gridUnit }
    }
}
