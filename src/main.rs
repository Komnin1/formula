//! Formula: a desktop chemical structure editor.

// On Windows the release build opens without an extra console window.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod arrange;
mod arrows;
mod bridged;
mod cdx;
mod clip;
mod compounds;
mod chem;
mod cip;
mod doc;
mod elements;
mod emf;
mod export;
mod formats;
mod hetero;
mod hotkeys;
mod icons;
mod iupac;
mod macapp;
mod names;
mod nmr;
mod nmrlearn;
mod ole;
mod props;
mod recover;
mod render;
mod styles;
mod sugars;
mod templates;
mod sequence;
mod xml;
#[cfg(test)]
mod tests;
#[cfg(all(test, windows))]
mod tests_windows;

use std::path::PathBuf;

fn main() -> eframe::Result {
    recover::install_panic_hook();
    let mut shot: Option<PathBuf> = None;
    let mut open: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--screenshot" => shot = args.next().map(PathBuf::from),
            "--open" => open = args.next().map(PathBuf::from),
            "--convert" => {
                args.next();
                args.next();
            }
            "--style" | "--tab" | "--templates" | "--stationery" | "--also-open" => {
                args.next();
            }
            s if ole::is_embedding_arg(s) => {}
            s if !s.starts_with("--") => open = Some(PathBuf::from(s)),
            _ => {}
        }
    }
    // `--convert in out`: change a file's format without opening a window.
    let argv: Vec<String> = std::env::args().collect();
    if let Some(i) = argv.iter().position(|a| a == "--convert") {
        let (Some(input), Some(output)) = (argv.get(i + 1), argv.get(i + 2)) else {
            eprintln!("usage: formula --convert <input> <output>");
            std::process::exit(2);
        };
        let mut doc = match app::read_any(std::path::Path::new(input)) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("could not read {}: {}", input, e);
                std::process::exit(1);
            }
        };
        if let Some(n) = argv.iter().position(|a| a == "--style").and_then(|k| argv.get(k + 1)).and_then(|n| styles::find(n)) {
            styles::apply(&mut doc, styles::to_style(n, true));
        }
        if let Err(e) = app::write_doc_file(&doc, std::path::Path::new(output)) {
            eprintln!("could not write {}: {}", output, e);
            std::process::exit(1);
        }
        return Ok(());
    }
    // Windows: make "Formula Structure" objects known (`--register` / `--unregister` only do that).
    if std::env::args().any(|a| a == "--register") {
        if let Err(e) = ole::register() {
            eprintln!("could not register: {}", e);
            std::process::exit(1);
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--unregister") {
        ole::unregister();
        return Ok(());
    }
    // Started by Windows to edit an object in a document (Word double-click).
    let embedding = std::env::args().skip(1).any(|a| ole::is_embedding_arg(&a));
    if embedding {
        ole::start_server();
    } else if shot.is_none() {
        let _ = ole::register();
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(app::APP_NAME)
            .with_app_id(app::app_id())
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([760.0, 480.0])
            // For an object the window stays hidden until the document asks to edit it: programs
            // such as LibreOffice start Formula briefly just to read an object when pasting.
            .with_visible(!embedding)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).unwrap_or_default()),
        // On macOS and Windows the app reads and writes the clipboard itself, so pictures with
        // the structure inside can be copied to and from other programs.
        clipboard_shortcuts: !clip::NATIVE,
        ..Default::default()
    };
    let result = eframe::run_native(app::APP_NAME, options, Box::new(move |cc| {
        let first = app::App::new(cc, shot, open);
        Ok(Box::new(app::Windows::new(first, &cc.egui_ctx)))
    }));
    ole::stop_server();
    result
}
