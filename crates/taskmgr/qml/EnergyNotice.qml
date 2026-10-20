import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// One quiet line under the CPU graph; the explanation is in the tooltip.
RowLayout {
    id: notice

    required property var monitor
    property var energy: null

    readonly property string request: monitor.energyRequest
    readonly property bool failed: request !== "" && request !== "pending" && request !== "granted"

    visible: energy?.status === "denied"
    spacing: Kirigami.Units.smallSpacing

    Kirigami.Icon {
        source: Icons.of("warning", Kirigami.Theme.textColor)
        opacity: 0.7
        Layout.preferredWidth: Kirigami.Units.iconSizes.small
        Layout.preferredHeight: Kirigami.Units.iconSizes.small
    }
    QQC2.Label {
        text: request === "pending" ? qsTr("Waiting for the administrator password…")
            : failed ? Tr.core(request)
            : qsTr("CPU power unavailable: the counter is restricted to root.")
        color: failed ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
        opacity: failed ? 1 : 0.7
        elide: Text.ElideRight
        Layout.fillWidth: true
    }
    QQC2.Button {
        flat: true
        text: qsTr("Allow access…")
        icon.source: Icons.of("admin", Kirigami.Theme.textColor)
        enabled: notice.request !== "pending"
        onClicked: notice.monitor.requestEnergyAccess()
        QQC2.ToolTip.text: qsTr("Lets your user read the RAPL counter until the next reboot (see docs/energy.md).")
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
    }
}
