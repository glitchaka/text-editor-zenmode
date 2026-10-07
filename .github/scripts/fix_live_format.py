from pathlib import Path


def replace_exact(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"No se encontro bloque requerido: {label}")
    return text.replace(old, new, 1)


app_path = Path("src/app.rs")
app = app_path.read_text(encoding="utf-8")

app = replace_exact(
    app,
    '''            visible: root.editor-active
                && (!root.zen-active
                    || bottom-reveal.has-hover
                    || island-hover.has-hover
                    || root.symbols-open
                    || root.pomodoro-text != "");''',
    '''            visible: !root.zen-active
                || bottom-reveal.has-hover
                || island-hover.has-hover
                || root.symbols-open;''',
    "isla visible fuera de Zenmode",
)

app = replace_exact(
    app,
    '''    fn start_pomodoro(&mut self) {
        if self.editor.is_none() || self.pomodoro_active {
            self.pomodoro_offer = false;
            return;
        }
        self.pomodoro_offer = false;
        self.pomodoro_active = true;''',
    '''    fn start_pomodoro(&mut self) {
        if self.pomodoro_active {
            self.pomodoro_offer = false;
            return;
        }
        if self.editor.is_none() {
            self.pomodoro_requested = true;
            self.pomodoro_offer = false;
            self.dirty = true;
            return;
        }
        self.pomodoro_requested = false;
        self.pomodoro_offer = false;
        self.pomodoro_active = true;''',
    "Pomodoro solicitado desde launcher",
)

app = replace_exact(
    app,
    '''                } else if modified.is_some() && modified != self.source_modified {
                    self.source_modified = modified;
                    if let Err(error) = editor.refresh_after_formatting() {
                        self.launcher.message =
                            Some(format!("No se pudo refrescar el formato: {error}"));
                    }
                }''',
    '''                } else if modified.is_some() && modified != self.source_modified {
                    self.source_modified = modified;
                    // El LSP solicita por si mismo el refresco de semantic tokens.
                    // Evitar cambiar prose -> markdown -> prose aqui: ese ciclo rompia
                    // el estado visual/seleccion justo despues de aplicar F2.
                    self.dirty = true;
                }''',
    "watcher de formato sin reiniciar lenguaje",
)

app = replace_exact(
    app,
    '''            let zen = model.zen_engaged();
            ui.set_editor_active(model.editor.is_some());
            ui.set_page_label(model.page_profile.paper.label().into());''',
    '''            let zen = model.zen_engaged();
            ui.set_editor_active(model.editor.is_some());
            ui.set_pomodoro_prompt(model.pomodoro_offer);
            ui.set_pomodoro_break(model.pomodoro_break);
            ui.set_pomodoro_text(model.pomodoro_label().into());
            ui.set_page_label(model.page_profile.paper.label().into());''',
    "refresco autonomo del cronometro",
)

app_path.write_text(app, encoding="utf-8", newline="\n")

spell_path = Path("src/spell.rs")
spell = spell_path.read_text(encoding="utf-8")

spell = replace_exact(
    spell,
    '''    let mut server = SpellServer::new(user_dictionary, library_root, source_file)?;
    let stdin = io::stdin();''',
    '''    let mut server = SpellServer::new(user_dictionary, library_root, source_file)?;
    let mut semantic_refresh_serial = 0u64;
    let stdin = io::stdin();''',
    "contador de solicitudes semantic refresh",
)

spell = replace_exact(
    spell,
    '''                if changed && command == Some(ADD_WORD_COMMAND) {
                    let documents = server
                        .documents
                        .iter()
                        .map(|(uri, text)| (uri.clone(), text.clone()))
                        .collect::<Vec<_>>();
                    for (uri, text) in documents {
                        server.publish(&uri, &text, &mut output)?;
                    }
                }
            }''',
    '''                if changed && command == Some(ADD_WORD_COMMAND) {
                    let documents = server
                        .documents
                        .iter()
                        .map(|(uri, text)| (uri.clone(), text.clone()))
                        .collect::<Vec<_>>();
                    for (uri, text) in documents {
                        server.publish(&uri, &text, &mut output)?;
                    }
                }
                if changed && command == Some(FORMAT_COMMAND) {
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
            }''',
    "refresco LSP inmediato tras F2",
)

spell_path.write_text(spell, encoding="utf-8", newline="\n")

cargo_path = Path("Cargo.toml")
cargo = cargo_path.read_text(encoding="utf-8")
cargo = replace_exact(
    cargo,
    'version = "0.2.14"',
    'version = "0.2.15"',
    "version 0.2.15",
)
cargo_path.write_text(cargo, encoding="utf-8", newline="\n")
