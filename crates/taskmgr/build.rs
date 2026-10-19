use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("org.taskmgr").qml_files([
        QmlFile::from("qml/Main.qml"),
        QmlFile::from("qml/Format.qml").singleton(true),
        QmlFile::from("qml/PerformancePage.qml"),
        QmlFile::from("qml/ResourceCard.qml"),
        QmlFile::from("qml/UsageChart.qml"),
        QmlFile::from("qml/Stat.qml"),
        QmlFile::from("qml/DetailRow.qml"),
        QmlFile::from("qml/ProcessesPage.qml"),
        QmlFile::from("qml/SystemPage.qml"),
    ]))
    .qt_module("Gui")
    .qt_module("Widgets")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .file("src/monitor.rs")
    .cpp_file("src/cpp/app.cpp")
    .build();
}
