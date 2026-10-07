from pathlib import Path


def replace_exact(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"No se encontro bloque requerido: {label}")
    return text.replace(old, new, 1)


app_path = Path("src/app.rs")
app = app_path.read_text(encoding="utf-8")
app = replace_exact(
    app,
    '''                } else if modified.is_some() && modified != self.source_modified {
                    self.source_modified = modified;
                    // El LSP solicita por si mismo el refresco de semantic tokens.
                    // Evitar cambiar prose -> markdown -> prose aqui: ese ciclo rompia
                    // el estado visual/seleccion justo despues de aplicar F2.
                    self.dirty = true;
                }''',
    '''                } else if modified.is_some() && modified != self.source_modified {
                    self.source_modified = modified;
                    if let Err(error) = editor.refresh_after_formatting() {
                        self.launcher.message =
                            Some(format!("No se pudo refrescar el formato: {error}"));
                    }
                }''',
    "watcher del sidecar de formato",
)
app_path.write_text(app, encoding="utf-8", newline="\n")

editor_path = Path("src/editor.rs")
editor = editor_path.read_text(encoding="utf-8")
editor = replace_exact(
    editor,
    '''    pub fn refresh_after_formatting(&self) -> Result<()> {
        self.send_command(":set-language markdown")?;
        thread::sleep(Duration::from_millis(20));
        self.send_command(":set-language prose")
    }''',
    '''    pub fn refresh_after_formatting(&self) -> Result<()> {
        // Helix 25.07.1 no implementa workspace/semanticTokens/refresh.
        // Reiniciar solo el LSP conserva el buffer y la seleccion, y fuerza
        // una nueva peticion de semantic tokens sin cambiar el lenguaje.
        self.send_command(":lsp-restart")
    }''',
    "refresco de formato mediante lsp-restart",
)
editor_path.write_text(editor, encoding="utf-8", newline="\n")

spell_path = Path("src/spell.rs")
spell = spell_path.read_text(encoding="utf-8")
spell = replace_exact(
    spell,
    '''    let mut server = SpellServer::new(user_dictionary, library_root, source_file)?;
    let mut semantic_refresh_serial = 0u64;
    let stdin = io::stdin();''',
    '''    let mut server = SpellServer::new(user_dictionary, library_root, source_file)?;
    let stdin = io::stdin();''',
    "retirar contador de refresh no soportado",
)
spell = replace_exact(
    spell,
    '''                if changed && command == Some(FORMAT_COMMAND) {
                    semantic_refresh_serial = semantic_refresh_serial.wrapping_add(1);
                    send_json(
                        &mut output,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": format!("helix-sst-semantic-refresh-{semantic_refresh_serial}"),
                            "method": "workspace/semanticTokens/refresh",
                            "params": null
                        }),
                    )?;
                }
''',
    '',
    "retirar workspace semanticTokens refresh no soportado",
)
spell_path.write_text(spell, encoding="utf-8", newline="\n")

cargo_path = Path("Cargo.toml")
cargo = cargo_path.read_text(encoding="utf-8")
cargo = replace_exact(
    cargo,
    'version = "0.2.15"',
    'version = "0.2.16"',
    "version 0.2.16",
)
cargo_path.write_text(cargo, encoding="utf-8", newline="\n")
