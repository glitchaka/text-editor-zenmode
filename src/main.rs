#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod document;
mod editor;
mod export;
mod library;
mod pty_protocol;
mod spell;

use std::{
    io::{Read, Write},
    path::PathBuf,
};

use anyhow::Result;

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    if args.first().map(String::as_str) == Some("--helix-sst-spell") {
        let path = args
            .get(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".spell-user"));
        let library_root = args.get(2).map(PathBuf::from);
        let source_file = args.get(3).map(PathBuf::from);
        std::process::exit(spell::run_lsp(path, library_root, source_file)?);
    }

    if args.first().map(String::as_str) == Some("--clipboard-get") {
        let mut clipboard = arboard::Clipboard::new()?;
        if let Ok(text) = clipboard.get_text() {
            std::io::stdout().write_all(text.as_bytes())?;
        }
        return Ok(());
    }

    if args.first().map(String::as_str) == Some("--clipboard-set") {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        let mut clipboard = arboard::Clipboard::new()?;
        clipboard.set_text(text)?;
        return Ok(());
    }

    let zen_requested = args.iter().any(|arg| arg == "--zen");
    let initial = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .map(PathBuf::from);

    app::run(initial, zen_requested)
}
