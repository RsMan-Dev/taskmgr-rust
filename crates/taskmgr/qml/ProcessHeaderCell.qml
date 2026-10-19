import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

// Column header with the machine total above the title.
QQC2.ItemDelegate {
    id: cell
    property string column
    property string title
    property string total: ""
    property string tip: ""
    property int alignment: Qt.AlignRight
    property bool sorted: false
    property bool descending: false

    signal sortRequested()
    signal columnsMenuRequested()

    padding: 0
    implicitHeight: Kirigami.Units.gridUnit * 3
    onClicked: sortRequested()
    QQC2.ToolTip.text: (tip ? tip + "\n" : "") + qsTr("Right click: choose columns")
    QQC2.ToolTip.visible: hovered
    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay

    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: cell.columnsMenuRequested()
    }
    TapHandler {
        acceptedDevices: PointerDevice.TouchScreen
        onLongPressed: cell.columnsMenuRequested()
    }
    background: Rectangle {
        color: cell.hovered ? Qt.alpha(Kirigami.Theme.highlightColor, 0.08) : "transparent"
    }

    contentItem: ColumnLayout {
        spacing: 0
        QQC2.Label {
            text: cell.total
            visible: cell.total !== ""
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.35
            font.features: { "tnum": 1 }
            Layout.alignment: cell.alignment
            Layout.leftMargin: cell.alignment === Qt.AlignLeft ? Kirigami.Units.gridUnit : Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
        }
        Item {
            Layout.fillHeight: true
        }
        RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.alignment: cell.alignment
            Layout.leftMargin: cell.alignment === Qt.AlignLeft ? Kirigami.Units.gridUnit : Kirigami.Units.largeSpacing
            Layout.rightMargin: Kirigami.Units.largeSpacing
            Layout.bottomMargin: Kirigami.Units.smallSpacing
            Kirigami.Icon {
                visible: cell.sorted
                source: Icons.of(cell.descending ? "sort-down" : "sort-up", Kirigami.Theme.textColor)
                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                Layout.preferredHeight: Kirigami.Units.iconSizes.small
            }
            QQC2.Label {
                text: cell.title
                opacity: 0.75
            }
        }
    }
}
