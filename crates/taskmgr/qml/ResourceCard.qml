import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Entry of the resource list: thumbnail graph, name and current value.
QQC2.ItemDelegate {
    id: card

    required property var resource

    property bool selected: false

    padding: Kirigami.Units.smallSpacing * 2

    // A solid accent fill would hide the CPU graph, drawn in the accent colour.
    background: Rectangle {
        radius: Kirigami.Units.cornerRadius
        color: card.selected ? Qt.alpha(Kirigami.Theme.highlightColor, 0.2)
             : card.hovered ? Qt.alpha(Kirigami.Theme.highlightColor, 0.08) : "transparent"
        border.width: card.selected || card.visualFocus ? 1 : 0
        border.color: Kirigami.Theme.highlightColor
    }

    contentItem: RowLayout {
        spacing: Kirigami.Units.largeSpacing

        UsageChart {
            compact: true
            series: card.resource.series
            maximum: card.resource.maximum
            accent: card.resource.color
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
            Layout.preferredHeight: Kirigami.Units.gridUnit * 2.75
        }

        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true

            QQC2.Label {
                text: card.resource.title
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.15
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            QQC2.Label {
                text: card.resource.subtitle
                visible: text !== ""
                opacity: 0.7
                font: Kirigami.Theme.smallFont
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            QQC2.Label {
                text: card.resource.summary
                opacity: 0.7
                font: Kirigami.Theme.smallFont
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }
}
