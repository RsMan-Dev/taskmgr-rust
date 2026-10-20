import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Tinted like the Windows heat map, in the theme's warning hue.
Rectangle {
    property real value: 0
    property real reference: 1
    property alias text: label.text
    property bool emphasized: false
    // Grey out nothing-to-see values when the settings ask for it.
    property bool zero: false

    readonly property real load: Math.min(1, Math.max(0, value / reference))

    // No colour animation: hundreds of cells fading at every refresh kept
    // the window repainting a quarter of the time.
    color: Qt.alpha(Kirigami.Theme.neutralTextColor, load > 0.01 ? 0.06 + 0.4 * Math.sqrt(load) : 0)

    Rectangle {
        visible: Prefs.columnSeparators
        width: 1
        height: parent.height
        color: Qt.alpha(Kirigami.Theme.textColor, 0.12)
    }

    QQC2.Label {
        id: label
        anchors.fill: parent
        anchors.rightMargin: Kirigami.Units.largeSpacing
        horizontalAlignment: Text.AlignRight
        verticalAlignment: Text.AlignVCenter
        font.weight: parent.emphasized ? Font.DemiBold : Font.Normal
        opacity: parent.zero && Prefs.dimZeros ? 0.4 : 1
        font.features: { "tnum": 1 }
        elide: Text.ElideLeft
    }
}
