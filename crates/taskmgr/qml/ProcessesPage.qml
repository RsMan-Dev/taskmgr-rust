import QtQuick
import org.kde.kirigami as Kirigami

// Grouped process list: comes with the identity resolver.
Item {
    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        icon.name: "view-process-tree"
        text: "Processus"
        explanation: "La liste des processus regroupés par application arrive avec le résolveur d'identité (Flatpak, Wine/Proton, AppImage…)."
    }
}
