import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// systemd journal of a service, a process or a moment, oldest first.
QQC2.Dialog {
    id: dialog

    required property var monitor
    property var query: ({})
    property string tag: ""
    property var answer: null

    readonly property var shown: {
        const all = answer?.entries ?? []
        const q = filter.text.trim().toLowerCase()
        return all.filter(e => (!errorsOnly.checked || e.priority <= 4)
            && (q === "" || e.message.toLowerCase().includes(q) || e.source.toLowerCase().includes(q)))
    }

    function show(title, q) {
        dialog.title = title
        query = q
        filter.text = ""
        children.checked = !!q.children
        reload()
        open()
    }

    function reload() {
        tag = String(Date.now())
        answer = null
        monitor.requestJournal(JSON.stringify(query), tag)
    }

    function stamp(us) {
        const d = new Date(us / 1000)
        const today = new Date().toDateString() === d.toDateString()
        return Qt.formatDateTime(d, today ? "HH:mm:ss" : "dd/MM HH:mm:ss")
    }

    function line(e) {
        return stamp(e.time_us) + "  " + e.source + (e.pid ? "[" + e.pid + "]" : "") + "  " + e.message
    }

    parent: QQC2.Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(parent.width - Kirigami.Units.gridUnit * 2, Kirigami.Units.gridUnit * 60)
    height: parent.height - Kirigami.Units.gridUnit * 3
    modal: true
    standardButtons: QQC2.Dialog.Close

    Connections {
        target: dialog.monitor
        function onJournalChanged() {
            const r = JSON.parse(dialog.monitor.journal)
            if (r.tag === dialog.tag) {
                dialog.answer = r
                Qt.callLater(() => list.positionViewAtEnd())
            }
        }
    }

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing

        RowLayout {
            Kirigami.SearchField {
                id: filter
                placeholderText: qsTr("Filter messages")
                Layout.fillWidth: true
            }
            QQC2.CheckBox {
                id: children
                visible: (dialog.query.pids ?? []).length > 0
                text: qsTr("Include child processes")
                onToggled: {
                    dialog.query = Object.assign({}, dialog.query, { children: checked })
                    dialog.reload()
                }
                QQC2.ToolTip.text: qsTr("Adds what the running processes started by this one logged.")
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
            QQC2.CheckBox {
                id: errorsOnly
                text: qsTr("Warnings and errors only")
            }
            QQC2.Button {
                text: qsTr("Refresh")
                icon.source: Icons.of("refresh", Kirigami.Theme.textColor)
                onClicked: dialog.reload()
            }
            QQC2.Button {
                text: qsTr("Copy")
                icon.source: Icons.of("copy", Kirigami.Theme.textColor)
                enabled: dialog.shown.length > 0
                onClicked: {
                    clipboard.text = dialog.shown.map(dialog.line).join("\n")
                    clipboard.selectAll()
                    clipboard.copy()
                }
            }
        }

        ThinScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ListView {
                id: list
                clip: true
                model: dialog.shown
                reuseItems: true
                boundsBehavior: Flickable.StopAtBounds

                delegate: RowLayout {
                    required property var modelData
                    width: ListView.view.width
                    spacing: Kirigami.Units.largeSpacing

                    QQC2.Label {
                        text: dialog.stamp(modelData.time_us)
                        opacity: 0.6
                        font: Kirigami.Theme.fixedWidthFont
                        Layout.alignment: Qt.AlignTop
                    }
                    QQC2.Label {
                        text: modelData.source + (modelData.pid ? "[" + modelData.pid + "]" : "")
                        opacity: 0.75
                        elide: Text.ElideRight
                        Layout.preferredWidth: Kirigami.Units.gridUnit * 9
                        Layout.alignment: Qt.AlignTop
                    }
                    QQC2.Label {
                        text: modelData.message
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font: Kirigami.Theme.fixedWidthFont
                        color: modelData.priority <= 3 ? Kirigami.Theme.negativeTextColor
                             : modelData.priority === 4 ? Kirigami.Theme.neutralTextColor : Kirigami.Theme.textColor
                        Layout.fillWidth: true
                    }
                }

                Kirigami.PlaceholderMessage {
                    anchors.centerIn: parent
                    width: parent.width - Kirigami.Units.gridUnit * 4
                    visible: list.count === 0
                    text: dialog.answer === null ? qsTr("Reading the journal…")
                        : dialog.answer.error !== "" ? Tr.core(dialog.answer.error)
                        : qsTr("No message")
                    explanation: dialog.answer !== null && dialog.answer.error === "" && (dialog.answer.entries?.length ?? 0) === 0
                        ? qsTr("Nothing was logged, or the journal of other users is not readable by your account.") : ""
                }
            }
        }
    }

    TextEdit {
        id: clipboard
        visible: false
    }
}
