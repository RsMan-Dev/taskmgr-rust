use std::error::Error;
use std::path::PathBuf;

use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn main() -> Result<(), Box<dyn Error>> {
    let mut files: Vec<_> = std::fs::read_dir("qml")?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "qml"))
        .collect();
    files.sort();
    println!("cargo::rerun-if-changed=qml");
    let qml = files.into_iter().map(|p| {
        let singleton = p.file_stem().is_some_and(|s| {
            matches!(
                s.to_str(),
                Some("Format" | "Tr" | "Prefs" | "Icons" | "ResourceKit")
            )
        });
        QmlFile::from(p).singleton(singleton)
    });

    let mut builder = CxxQtBuilder::new_qml_module(QmlModule::new("org.taskmgr").qml_files(qml));
    if let Some(qrc) = translations() {
        builder = builder.qrc(qrc);
    }
    builder = builder.qrc(icons()?);
    builder
        .qt_module("Gui")
        .qt_module("Widgets")
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .file("src/monitor/mod.rs")
        .file("src/monitor/startup.rs")
        .cpp_file("src/cpp/app.cpp")
        .cpp_file("src/cpp/seriesplot.h")
        .cpp_file("src/cpp/seriesplot.cpp")
        .build();
    Ok(())
}

/// Compiles `translations/*.ts` with lrelease into a resource at `:/i18n`.
/// Without lrelease the app builds and runs in English.
fn translations() -> Option<PathBuf> {
    let dir = std::path::Path::new("../../translations");
    println!("cargo::rerun-if-changed={}", dir.display());
    let out = PathBuf::from(std::env::var("OUT_DIR").ok()?);
    let lrelease = find_lrelease()?;
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

/// LRELEASE, else Qt's own tool directories (outside PATH on most
/// distributions), else the names distributions give it.
fn find_lrelease() -> Option<String> {
    if let Ok(path) = std::env::var("LRELEASE") {
        return Some(path);
    }
    let qmake = std::env::var("QMAKE").unwrap_or_else(|_| "qmake6".to_owned());
    let dirs = ["QT_HOST_BINS", "QT_HOST_LIBEXECS", "QT_INSTALL_BINS"]
        .iter()
        .filter_map(|key| {
            let out = std::process::Command::new(&qmake)
                .args(["-query", key])
                .output()
                .ok()?;
            Some(String::from_utf8(out.stdout).ok()?.trim().to_owned())
        })
        .map(|dir| format!("{dir}/lrelease"));
    let names = ["lrelease", "lrelease-qt6", "lrelease6"].map(str::to_owned);
    dirs.chain(names).find(|tool| {
        std::process::Command::new(tool)
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

/// Bundled interface icons and the logo, served at `:/icons`. Each icon
/// comes in two inks (Breeze text colours) so any renderer, QStyle included,
/// can draw it; colouring at run time only works in Kirigami.Icon.
fn icons() -> Result<PathBuf, Box<dyn Error>> {
    println!("cargo::rerun-if-changed=icons");
    println!("cargo::rerun-if-changed=../../packaging/fr.rsman.taskmgr-rust.svg");
    let here = std::env::current_dir()?;
    let out = PathBuf::from(std::env::var("OUT_DIR")?);
    let mut entries = format!(
        "<file alias=\"logo.svg\">{}</file>",
        here.join("../../packaging/fr.rsman.taskmgr-rust.svg")
            .display()
    );
    let mut files: Vec<_> = std::fs::read_dir("icons")?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "svg"))
        .collect();
    files.sort();
    for (variant, ink) in [("on-dark", "#fcfcfc"), ("on-light", "#232629")] {
        let dir = out.join(variant);
        std::fs::create_dir_all(&dir)?;
        for f in &files {
            let Some(name) = f.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            let svg =
                std::fs::read_to_string(f)?.replacen("<svg ", &format!("<svg fill=\"{ink}\" "), 1);
            std::fs::write(dir.join(&name), svg)?;
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
    )?;
    Ok(qrc)
}
