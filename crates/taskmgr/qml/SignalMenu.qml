import QtQuick
import QtQuick.Controls as QQC2

// Less common signals, for a process or a whole application.
QQC2.Menu {
    id: menu

    signal chosen(string signal)

    title: qsTr("Send signal")

    Repeater {
        model: [
            ["stop", qsTr("Suspend (STOP)")],
            ["cont", qsTr("Resume (CONT)")],
            ["hup", qsTr("Hang up (HUP)")],
            ["int", qsTr("Interrupt (INT)")],
            ["usr1", qsTr("User 1 (USR1)")],
            ["usr2", qsTr("User 2 (USR2)")],
        ]
        QQC2.MenuItem {
            required property var modelData
            text: modelData[1]
            onTriggered: menu.chosen(modelData[0])
        }
    }
}
