import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Keyboard shortcuts of the window.
QQC2.Dialog {
    id: dialog

    readonly property var groups: [
        [qsTr("General"), [
            ["Ctrl+1 … Ctrl+9", qsTr("Go to a page")],
            ["Ctrl+,", qsTr("Settings")],
            ["Ctrl+F", qsTr("Search")],
            ["Ctrl+P", qsTr("Pause or resume updates")],
            ["F12", qsTr("Compact view, full screen")],
            ["F1", qsTr("Keyboard shortcuts")],
            ["Ctrl+Q", qsTr("Quit")],
        ]],
        [qsTr("Lists"), [
            [qsTr("Up, Down"), qsTr("Select a row")],
            [qsTr("Right, Left"), qsTr("Expand or collapse a group")],
            [qsTr("Delete"), qsTr("End the selected task")],
            ["Ctrl+C", qsTr("Copy the selected row")],
            [qsTr("Right click"), qsTr("Actions on a row, columns on a header")],
        ]],
    ]

    title: qsTr("Keyboard shortcuts")
    parent: QQC2.Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(parent.width - Kirigami.Units.gridUnit * 2, Kirigami.Units.gridUnit * 30)
    modal: true
    standardButtons: QQC2.Dialog.Close

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        Repeater {
            model: dialog.groups
            ColumnLayout {
                required property var modelData
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true

                Kirigami.Heading {
                    level: 4
                    text: modelData[0]
                }
                Repeater {
                    model: modelData[1]
                    RowLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        QQC2.Label {
                            text: modelData[1]
                            Layout.fillWidth: true
                        }
                        QQC2.Label {
                            text: modelData[0]
                            font.family: "monospace"
                            opacity: 0.8
                        }
                    }
                }
            }
        }
    }
}
