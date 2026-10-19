import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

// Read-only text that can be selected and copied (addresses, models,
// paths), with a right-click menu.
TextEdit {
    id: field

    readOnly: true
    selectByMouse: true
    persistentSelection: false
    textFormat: TextEdit.PlainText
    color: Kirigami.Theme.textColor
    selectionColor: Kirigami.Theme.highlightColor
    selectedTextColor: Kirigami.Theme.highlightedTextColor
    font: Kirigami.Theme.defaultFont

    function copyAll() {
        const [start, end] = [selectionStart, selectionEnd]
        selectAll()
        copy()
        select(start, end)
    }

    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: menu.popup()
    }

    QQC2.Menu {
        id: menu
        QQC2.MenuItem {
            text: qsTr("Copy")
            icon.source: Icons.of("copy", Kirigami.Theme.textColor)
            onTriggered: field.selectedText !== "" ? field.copy() : field.copyAll()
        }
        QQC2.MenuItem {
            text: qsTr("Select all")
            onTriggered: field.selectAll()
        }
    }
}
