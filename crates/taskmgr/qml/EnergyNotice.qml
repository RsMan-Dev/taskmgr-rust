import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Kirigami.InlineMessage {
    id: notice

    required property var monitor
    property var energy: null

    readonly property string request: monitor.energyRequest
    readonly property bool failed: request !== "" && request !== "pending" && request !== "granted"

    visible: energy?.status === "denied"
    type: failed ? Kirigami.MessageType.Error : Kirigami.MessageType.Warning
    text: request === "pending" ? qsTr("Waiting for the administrator password…")
        : failed ? Tr.core(request)
        : qsTr("CPU power unavailable: the counter is restricted to root.")

    actions: [
        Kirigami.Action {
            text: qsTr("Allow access…")
            icon.name: "security-high"
            enabled: notice.request !== "pending"
            tooltip: qsTr("Lets your user read the RAPL counter until the next reboot (see docs/energy.md).")
            onTriggered: notice.monitor.requestEnergyAccess()
        }
    ]
}
