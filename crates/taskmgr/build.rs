use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn main() {
    let mut files: Vec<_> = std::fs::read_dir("qml")
        .expect("qml directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "qml"))
        .collect();
    files.sort();
    println!("cargo::rerun-if-changed=qml");
    let qml = files.into_iter().map(|p| {
        let singleton = p.file_stem().is_some_and(|s| s == "Format");
        QmlFile::from(p).singleton(singleton)
    });

    CxxQtBuilder::new_qml_module(QmlModule::new("org.taskmgr").qml_files(qml))
        .qt_module("Gui")
        .qt_module("Widgets")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .file("src/monitor.rs")
        .cpp_file("src/cpp/app.cpp")
        .build();
}
