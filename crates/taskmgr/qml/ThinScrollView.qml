import QtQuick
import QtQuick.Controls as QQC2

// ScrollView with ThinScrollBar, beside the content rather than over it.
QQC2.ScrollView {
    id: view

    // The bar's room is kept even when it hides: following its visibility
    // would loop (less room, different content size, different visibility).
    rightPadding: QQC2.ScrollBar.vertical.policy === QQC2.ScrollBar.AlwaysOff ? 0 : QQC2.ScrollBar.vertical.thickness
    bottomPadding: QQC2.ScrollBar.horizontal.policy === QQC2.ScrollBar.AlwaysOff ? 0 : QQC2.ScrollBar.horizontal.thickness

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
