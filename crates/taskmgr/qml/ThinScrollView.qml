import QtQuick
import QtQuick.Controls as QQC2

// ScrollView with ThinScrollBar, beside the content rather than over it.
QQC2.ScrollView {
    id: view

    rightPadding: QQC2.ScrollBar.vertical.visible ? QQC2.ScrollBar.vertical.width : 0
    bottomPadding: QQC2.ScrollBar.horizontal.visible ? QQC2.ScrollBar.horizontal.height : 0

    QQC2.ScrollBar.vertical: ThinScrollBar {
        parent: view
        x: view.mirrored ? 0 : view.width - width
        y: view.topPadding
        height: view.availableHeight
    }
    QQC2.ScrollBar.horizontal: ThinScrollBar {
        parent: view
        x: view.leftPadding
        y: view.height - height
        width: view.availableWidth
    }
}
