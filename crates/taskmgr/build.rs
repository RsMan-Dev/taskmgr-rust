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
        let singleton = p
            .file_stem()
            .is_some_and(|s| matches!(s.to_str(), Some("Format" | "Tr" | "Prefs" | "Icons")));
        QmlFile::from(p).singleton(singleton)
    });

    let mut builder = CxxQtBuilder::new_qml_module(QmlModule::new("org.taskmgr").qml_files(qml));
    if let Some(qrc) = translations() {
        builder = builder.qrc(qrc);
    }
    builder = builder.qrc(icons());
    builder
        .qt_module("Gui")
        .qt_module("Widgets")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .file("src/monitor.rs")
        .cpp_file("src/cpp/app.cpp")
        .cpp_file("src/cpp/seriesplot.h")
        .cpp_file("src/cpp/seriesplot.cpp")
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

/// Bundled interface icons and the logo, served at `:/icons`. Each icon
/// comes in two inks (Breeze text colours) so any renderer, QStyle included,
/// can draw it; colouring at run time only works in Kirigami.Icon.
fn icons() -> std::path::PathBuf {
    println!("cargo::rerun-if-changed=icons");
    println!("cargo::rerun-if-changed=../../packaging/fr.rsman.taskmgr-rust.svg");
    let here = std::env::current_dir().expect("crate dir");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let mut entries = format!(
        "<file alias=\"logo.svg\">{}</file>",
        here.join("../../packaging/fr.rsman.taskmgr-rust.svg")
            .display()
    );
    let mut files: Vec<_> = std::fs::read_dir("icons")
        .expect("icons directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "svg"))
        .collect();
    files.sort();
    for (variant, ink) in [("on-dark", "#fcfcfc"), ("on-light", "#232629")] {
        let dir = out.join(variant);
        std::fs::create_dir_all(&dir).expect("icon variant dir");
        for f in &files {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            let svg = std::fs::read_to_string(f).expect("read icon").replacen(
                "<svg ",
                &format!("<svg fill=\"{ink}\" "),
                1,
            );
            std::fs::write(dir.join(&name), svg).expect("write icon");
            entries.push_str(&format!(
                "<file alias=\"{variant}/{name}\">{}</file>",
                dir.join(&name).display()
            ));
        }
    }
    let qrc = out.join("icons.qrc");
    std::fs::write(
        &qrc,
        format!("<RCC><qresource prefix=\"/icons\">{entries}</qresource></RCC>"),
    )
    .expect("write icons.qrc");
    qrc
}
