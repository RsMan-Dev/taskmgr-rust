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
    text: request === "pending" ? "En attente du mot de passe administrateur…"
        : failed ? request
        : "Consommation du processeur indisponible : le compteur est réservé à root."

    actions: [
        Kirigami.Action {
            text: "Autoriser l'accès…"
            icon.name: "security-high"
            enabled: notice.request !== "pending"
            tooltip: "Donne la lecture du compteur RAPL à votre utilisateur jusqu'au prochain redémarrage (voir docs/energy.md)."
            onTriggered: notice.monitor.requestEnergyAccess()
        }
    ]
}
