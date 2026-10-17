from pathlib import Path


def rep(text, old, new, label):
    if old not in text:
        raise SystemExit(f'missing anchor: {label}')
    return text.replace(old, new, 1)

p = Path('src/app.rs')
s = p.read_text(encoding='utf-8')
s = rep(s, '    creating: bool,\n    new_name: String,\n', '    creating: bool,\n    creating_folder: bool,\n    new_name: String,\n', 'folder field')
s = rep(s, '            creating: false,\n            new_name: String::new(),\n', '            creating: false,\n            creating_folder: false,\n            new_name: String::new(),\n', 'folder init')
s = rep(s, '                    !name.to_ascii_lowercase().ends_with(".zen.json")\n', '                    let lower = name.to_ascii_lowercase();\n                    !lower.ends_with(".zen.json")\n                        && !(lower.starts_with(".zen-project-") && lower.ends_with(".json"))\n', 'hide manifests')

anchor = '        if self.launcher.creating {\n'
block = r'''        if self.launcher.creating_folder {
            match key.code {
                KeyCode::Esc => {
                    self.launcher.creating_folder = false;
                    self.launcher.new_name.clear();
                    self.launcher.message = None;
                }
                KeyCode::Enter => {
                    let name = self.launcher.new_name.trim().to_owned();
                    if name.is_empty() {
                        self.launcher.message = Some("Escribe un nombre de carpeta.".into());
                    } else if name.contains('/') || name.contains('\\') {
                        self.launcher.message = Some("El nombre de la carpeta no puede contener / ni \\.".into());
                    } else {
                        let path = self.launcher.cwd.join(&name);
                        if path.exists() {
                            self.launcher.message = Some("Ya existe un archivo o carpeta con ese nombre.".into());
                        } else {
                            match fs::create_dir(&path) {
                                Ok(()) => {
                                    self.launcher.creating_folder = false;
                                    self.launcher.new_name.clear();
                                    self.launcher.message = Some(format!("Carpeta creada: {name}"));
                                    self.launcher.refresh();
                                }
                                Err(error) => self.launcher.message = Some(format!("No se pudo crear la carpeta: {error}")),
                            }
                        }
                    }
                }
                KeyCode::Backspace => { self.launcher.new_name.pop(); }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) && !matches!(ch, '\r' | '\n') => self.launcher.new_name.push(ch),
                _ => {}
            }
            self.render_launcher();
            return;
        }

'''
s = rep(s, anchor, block + anchor, 'folder handler')
s = rep(s, "            KeyCode::Char('r') | KeyCode::Char('R') => self.launcher.refresh(),\n", "            KeyCode::Char('f') | KeyCode::Char('F') => {\n                self.launcher.creating_folder = true;\n                self.launcher.new_name.clear();\n                self.launcher.message = None;\n            }\n            KeyCode::Char('r') | KeyCode::Char('R') => self.launcher.refresh(),\n", 'folder shortcut')
marker = '        } else if self.launcher.creating {\n'
render = r'''        } else if self.launcher.creating_folder {
            push_line(&mut out, &framed_left("", inner_width, "38;5;244"));
            push_line(&mut out, &framed_left(&format!("\x1b[1;38;5;222m{ICON_FOLDER}\x1b[0m   NUEVA CARPETA"), inner_width, "38;5;244"));
            push_line(&mut out, &framed_left(&format!("\x1b[38;5;250mNombre:\x1b[0m {}\x1b[?25h", self.launcher.new_name), inner_width, "38;5;244"));
            push_line(&mut out, &framed_left("", inner_width, "38;5;244"));
            push_line(&mut out, &framed_left("\x1b[38;5;244mEnter crear  ·  Esc cancelar\x1b[0m", inner_width, "38;5;244"));
'''
s = rep(s, marker, render + marker, 'folder render')
s = rep(s, '                    "\\x1b[38;5;244m↑↓ seleccionar  Enter abrir  N nuevo  I importar  E exportar  Z modo  Backspace subir  R refrescar\\x1b[0m",\n', '                    "\\x1b[38;5;244m↑↓ seleccionar  Enter abrir  N nuevo  F carpeta  P proyecto  I importar  E exportar  F11 pantalla  Backspace subir  R refrescar\\x1b[0m",\n', 'footer')
s = s.replace('"\\x1b[38;5;250mProyecto:\\x1b[0m {}\\x1b[?25h"', '"\\x1b[38;5;250mProyecto (nuevo o existente):\\x1b[0m {}\\x1b[?25h"')
p.write_text(s, encoding='utf-8')

p = Path('src/document.rs')
s = p.read_text(encoding='utf-8')
old = '''    write(path, &document)
        .with_context(|| format!("No se pudo actualizar el proyecto de {}", path.display()))
}
'''
new = '''    write(path, &document)
        .with_context(|| format!("No se pudo actualizar el proyecto de {}", path.display()))?;
    if let Some(root) = path.parent() {
        sync_project_manifest(root, &project)?;
    }
    Ok(())
}

fn project_manifest_path(root: &Path, project: &str) -> PathBuf {
    let safe = project.chars().map(|ch| if ch.is_alphanumeric() || matches!(ch, '-' | '_') { ch } else { '_' }).collect::<String>();
    root.join(format!(".zen-project-{}.json", if safe.is_empty() { "project" } else { &safe }))
}

fn sync_project_manifest(root: &Path, project: &str) -> Result<()> {
    let project = project.trim();
    if project.is_empty() { return Ok(()); }
    let documents = project_documents(root, project)?;
    let members = documents.iter().map(|(path, metadata)| json!({
        "path": path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
        "id": metadata.id,
        "title": metadata.title,
        "type": metadata.kind,
        "chapter": metadata.chapter,
        "order": metadata.order
    })).collect::<Vec<_>>();
    let manifest = json!({"format": 1, "type": "zen-project", "name": project, "documents": members});
    fs::write(project_manifest_path(root, project), serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}
'''
s = rep(s, old, new, 'project manifest')
p.write_text(s, encoding='utf-8')

p = Path('Cargo.toml')
s = p.read_text(encoding='utf-8')
s = rep(s, 'version = "0.2.19"', 'version = "0.2.20"', 'cargo version')
p.write_text(s, encoding='utf-8')
p = Path('Cargo.lock')
s = p.read_text(encoding='utf-8')
s = rep(s, 'name = "zen-editor"\nversion = "0.2.19"', 'name = "zen-editor"\nversion = "0.2.20"', 'lock version')
p.write_text(s, encoding='utf-8')
p = Path('CHANGELOG.md')
s = p.read_text(encoding='utf-8')
head = '# Historial de versiones — Zen Editor\n\nCambios por versión de Zen Editor.\n\n'
entry = '## v0.2.20 — 2026-10-17\n\n- Forzar repintado completo de Helix al cerrar menús F2 para eliminar restos visuales.\n- Añadir creación de carpetas desde el launcher con `F`.\n- Hacer explícita la asignación/creación de proyectos con `P`.\n- Generar un manifiesto `.zen-project-*.json` que agrupa los TXT/HSST separados de cada proyecto.\n\n'
if '## v0.2.20' not in s:
    s = rep(s, head, head + entry, 'changelog')
p.write_text(s, encoding='utf-8')
