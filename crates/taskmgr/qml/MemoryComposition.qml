import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// In use / cached / free as one segmented bar, like Windows' memory composition.
ColumnLayout {
    id: bar

    property var composition: null
    property color accent: Kirigami.Theme.highlightColor

    readonly property var parts: composition ? [
        { label: qsTr("In use"), value: composition.used, color: accent },
        { label: qsTr("Cached"), value: composition.cached, color: Qt.alpha(accent, 0.45) },
        { label: qsTr("Free"), value: composition.free, color: "transparent" },
    ] : []
    readonly property real sum: parts.reduce((a, p) => a + p.value, 0)

    spacing: Kirigami.Units.smallSpacing / 2

    QQC2.Label {
        text: qsTr("Memory composition")
        opacity: 0.7
        font: Kirigami.Theme.smallFont
    }

    Rectangle {
        color: "transparent"
        border.color: Qt.alpha(bar.accent, 0.6)
        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 1.6

        Row {
            anchors.fill: parent
            anchors.margins: 1
            Repeater {
                model: bar.parts
                Rectangle {
                    required property var modelData
                    width: bar.sum > 0 ? parent.width * modelData.value / bar.sum : 0
                    height: parent.height
                    color: modelData.color
                    border.color: Qt.alpha(bar.accent, 0.35)
                    QQC2.ToolTip.text: modelData.label + " : " + Format.bytes(modelData.value)
                    QQC2.ToolTip.visible: partHover.hovered
                    HoverHandler {
                        id: partHover
                    }
                }
            }
        }
    }

    RowLayout {
        spacing: Kirigami.Units.gridUnit
        Repeater {
            model: bar.parts
            QQC2.Label {
                required property var modelData
                text: modelData.label + " " + Format.bytes(modelData.value)
                opacity: 0.7
                font: Kirigami.Theme.smallFont
            }
        }
    }
}
