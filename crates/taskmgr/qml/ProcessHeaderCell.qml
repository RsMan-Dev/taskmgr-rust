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
    // A limit of the column (rights, sandbox), shown as an icon.
    property string notice: ""
    property string noticeIcon: "warning"
    property int alignment: Qt.AlignRight
    property bool sorted: false
    property bool descending: false

    signal sortRequested()
    signal columnsMenuRequested()
    signal noticeClicked()

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
            QQC2.ToolButton {
                visible: cell.notice !== ""
                icon.source: Icons.of(cell.noticeIcon, Kirigami.Theme.textColor)
                icon.width: Kirigami.Units.iconSizes.small
                icon.height: Kirigami.Units.iconSizes.small
                display: QQC2.AbstractButton.IconOnly
                flat: true
                padding: 0
                text: cell.notice
                onClicked: cell.noticeClicked()
                QQC2.ToolTip.text: cell.notice
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: 0
                Layout.preferredWidth: Kirigami.Units.iconSizes.small + 4
                Layout.preferredHeight: Kirigami.Units.iconSizes.small + 4
            }
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
