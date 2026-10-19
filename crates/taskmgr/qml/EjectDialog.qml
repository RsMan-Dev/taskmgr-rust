import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Safe removal of a drive, with the processes that keep it busy.
QQC2.Dialog {
    id: dialog

    required property var monitor
    property string block: ""
    readonly property var answer: monitor.ejectResult !== "" ? JSON.parse(monitor.ejectResult) : null

    function eject(name, label) {
        block = name
        title = qsTr("Eject %1").arg(label)
        monitor.ejectDisk(name)
        open()
    }

    parent: QQC2.Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(parent.width - Kirigami.Units.gridUnit * 2, Kirigami.Units.gridUnit * 36)
    modal: true
    standardButtons: QQC2.Dialog.Close

    ColumnLayout {
        anchors.fill: parent
        spacing: Kirigami.Units.largeSpacing

        QQC2.BusyIndicator {
            visible: !dialog.answer
            Layout.alignment: Qt.AlignHCenter
        }
        QQC2.Label {
            visible: !!dialog.answer
            wrapMode: Text.Wrap
            Layout.fillWidth: true
            text: !dialog.answer ? ""
                : dialog.answer.ok ? (dialog.answer.message ? Tr.core(dialog.answer.message) : qsTr("The drive can be removed safely."))
                : dialog.answer.blockers.length > 0 ? qsTr("The drive is in use by these processes. Close them, then try again.")
                : qsTr("The drive could not be ejected: %1").arg(Tr.core(dialog.answer.message))
        }
        Repeater {
            model: dialog.answer?.blockers ?? []
            RowLayout {
                required property var modelData
                Layout.fillWidth: true
                QQC2.Label {
                    text: modelData.name + "  (" + modelData.pid + ")"
                    font.bold: true
                }
                QQC2.Label {
                    text: modelData.path
                    elide: Text.ElideMiddle
                    opacity: 0.7
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    text: qsTr("End task")
                    onClicked: dialog.monitor.sendSignal(String(modelData.pid), "term")
                }
            }
        }
        QQC2.Button {
            visible: !!dialog.answer && !dialog.answer.ok
            text: qsTr("Try again")
            Layout.alignment: Qt.AlignRight
            onClicked: dialog.monitor.ejectDisk(dialog.block)
        }
    }
}
