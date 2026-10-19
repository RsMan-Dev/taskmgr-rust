import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

RowLayout {
    property alias title: heading.text
    default property alias actions: actionRow.data

    spacing: Kirigami.Units.largeSpacing
    Layout.fillWidth: true
    Layout.leftMargin: Kirigami.Units.gridUnit
    Layout.rightMargin: Kirigami.Units.gridUnit
    Layout.topMargin: Kirigami.Units.gridUnit
    Layout.bottomMargin: Kirigami.Units.smallSpacing

    Kirigami.Heading {
        id: heading
        level: 1
        Layout.fillWidth: true
        Layout.minimumWidth: implicitWidth
    }

    RowLayout {
        id: actionRow
        spacing: Kirigami.Units.smallSpacing
    }
}
