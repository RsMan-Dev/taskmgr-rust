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
        let singleton = p.file_stem().is_some_and(|s| s == "Format" || s == "Tr");
        QmlFile::from(p).singleton(singleton)
    });

    let mut builder = CxxQtBuilder::new_qml_module(QmlModule::new("org.taskmgr").qml_files(qml));
    if let Some(qrc) = translations() {
        builder = builder.qrc(qrc);
    }
    builder
        .qt_module("Gui")
        .qt_module("Widgets")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .file("src/monitor.rs")
        .cpp_file("src/cpp/app.cpp")
        .build();
}

/// Compiles `translations/*.ts` with lrelease into a resource at `:/i18n`.
/// Without lrelease the app builds and runs in English.
fn translations() -> Option<std::path::PathBuf> {
    let dir = std::path::Path::new("../../translations");
    println!("cargo::rerun-if-changed={}", dir.display());
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").ok()?);
    let lrelease = std::env::var("LRELEASE").unwrap_or_else(|_| "lrelease".to_owned());
    let mut entries = String::new();
    for ts in std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
    {
        if ts.extension().is_none_or(|x| x != "ts") {
            continue;
        }
        let qm = out.join(ts.with_extension("qm").file_name()?);
        let ok = std::process::Command::new(&lrelease)
            .arg("-silent")
            .arg(&ts)
            .arg("-qm")
            .arg(&qm)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            println!("cargo::warning=lrelease failed or missing: no translations");
            return None;
        }
        entries.push_str(&format!(
            "<file>{}</file>",
            qm.file_name()?.to_string_lossy()
        ));
    }
    let qrc = out.join("i18n.qrc");
    std::fs::write(
        &qrc,
        format!("<RCC><qresource prefix=\"/i18n\">{entries}</qresource></RCC>"),
    )
    .ok()?;
    Some(qrc)
}
