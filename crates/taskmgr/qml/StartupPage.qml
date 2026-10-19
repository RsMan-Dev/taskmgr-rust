import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Applications started at login (XDG autostart).
QQC2.ScrollView {
    id: page

    required property var monitor
    property bool active: false
    property string message: ""

    readonly property var entries: monitor.autostart !== "" ? JSON.parse(monitor.autostart) : []

    contentWidth: availableWidth
    onActiveChanged: if (active) monitor.refreshAutostart()

    ColumnLayout {
        width: page.availableWidth
        spacing: Kirigami.Units.largeSpacing

        PageHeader {
            title: qsTr("Startup apps")

            QQC2.Button {
                text: qsTr("Refresh")
                icon.source: Icons.of("refresh", Kirigami.Theme.textColor)
                display: QQC2.AbstractButton.IconOnly
                onClicked: page.monitor.refreshAutostart()
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
            }
        }

        Kirigami.InlineMessage {
            visible: page.message !== ""
            text: page.message
            type: Kirigami.MessageType.Error
            showCloseButton: true
            onVisibleChanged: if (!visible) page.message = ""
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.gridUnit
            Layout.rightMargin: Kirigami.Units.gridUnit
        }

        Kirigami.PlaceholderMessage {
            visible: page.entries.length === 0
            icon.source: Icons.of("nav-startup", Kirigami.Theme.textColor)
            text: qsTr("No application starts at login")
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.gridUnit * 4
        }

        Repeater {
            model: page.entries

            Card {
                id: card
                required property var modelData
                Layout.leftMargin: Kirigami.Units.gridUnit
                Layout.rightMargin: Kirigami.Units.gridUnit
                implicitHeight: row.implicitHeight + Kirigami.Units.largeSpacing * 2
                opacity: modelData.applies ? 1 : 0.6

                RowLayout {
                    id: row
                    anchors.fill: parent
                    anchors.leftMargin: Kirigami.Units.gridUnit
                    anchors.rightMargin: Kirigami.Units.gridUnit
                    spacing: Kirigami.Units.largeSpacing

                    Kirigami.Icon {
                        source: card.modelData.icon || Icons.of("app", Kirigami.Theme.textColor)
                        fallback: Icons.of("app", Kirigami.Theme.textColor)
                        Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                        Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        QQC2.Label {
                            text: card.modelData.name
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        QQC2.Label {
                            text: card.modelData.command
                            opacity: 0.6
                            font: Kirigami.Theme.smallFont
                            elide: Text.ElideMiddle
                            Layout.fillWidth: true
                        }
                        QQC2.Label {
                            visible: !card.modelData.applies
                            text: qsTr("Not started on this desktop")
                            opacity: 0.6
                            font: Kirigami.Theme.smallFont
                        }
                    }
                    QQC2.Label {
                        text: card.modelData.system ? qsTr("System") : qsTr("User")
                        opacity: 0.6
                    }
                    QQC2.Switch {
                        checked: card.modelData.enabled
                        onToggled: {
                            const error = page.monitor.setAutostartEnabled(card.modelData.id, checked)
                            page.message = error === "" ? "" : error
                        }
                        QQC2.ToolTip.text: checked ? qsTr("Started at login") : qsTr("Not started at login")
                        QQC2.ToolTip.visible: hovered
                    }
                }
            }
        }

        Item {
            Layout.preferredHeight: Kirigami.Units.gridUnit
        }
    }
}
