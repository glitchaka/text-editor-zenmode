from pathlib import Path


def must(text, old, new, label):
    if old not in text:
        raise SystemExit(f'missing core anchor: {label}')
    return text.replace(old, new, 1)

p = Path('src/app.rs')
s = p.read_text(encoding='utf-8')
s = must(s, '    creating: bool,\n    new_name: String,\n', '    creating: bool,\n    creating_folder: bool,\n    new_name: String,\n', 'folder field')
s = must(s, '            creating: false,\n            new_name: String::new(),\n', '            creating: false,\n            creating_folder: false,\n            new_name: String::new(),\n', 'folder init')
s = must(s, '                    !name.to_ascii_lowercase().ends_with(".zen.json")\n', '                    let lower = name.to_ascii_lowercase();\n                    !lower.ends_with(".zen.json")\n                        && !(lower.starts_with(".zen-project-") && lower.ends_with(".json"))\n', 'hide project manifests')

anchor = '        if self.launcher.creating {\n'
block = '''        if self.launcher.creating_folder {\n            match key.code {\n                KeyCode::Esc => {\n                    self.launcher.creating_folder = false;\n                    self.launcher.new_name.clear();\n                    self.launcher.message = None;\n                }\n                KeyCode::Enter => {\n                    let name = self.launcher.new_name.trim().to_owned();\n                    if name.is_empty() {\n                        self.launcher.message = Some("Escribe un nombre de carpeta.".into());\n                    } else if name.contains('/') || name.contains('\\\\') {\n                        self.launcher.message = Some("El nombre de la carpeta no puede contener / ni \\\\.".into());\n                    } else {\n                        let path = self.launcher.cwd.join(&name);\n                        if path.exists() {\n                            self.launcher.message = Some("Ya existe un archivo o carpeta con ese nombre.".into());\n                        } else {\n                            match fs::create_dir(&path) {\n                                Ok(()) => {\n                                    self.launcher.creating_folder = false;\n                                    self.launcher.new_name.clear();\n                                    self.launcher.message = Some(format!("Carpeta creada: {name}"));\n                                    self.launcher.refresh();\n                                }\n                                Err(error) => self.launcher.message = Some(format!("No se pudo crear la carpeta: {error}")),\n                            }\n                        }\n                    }\n                }\n                KeyCode::Backspace => { self.launcher.new_name.pop(); }\n                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) && !matches!(ch, '\\r' | '\\n') => self.launcher.new_name.push(ch),\n                _ => {}\n            }\n            self.render_launcher();\n            return;\n        }\n\n'''
s = must(s, anchor, block + anchor, 'folder handler')
s = must(s, "            KeyCode::Char('r') | KeyCode::Char('R') => self.launcher.refresh(),\n", "            KeyCode::Char('f') | KeyCode::Char('F') => {\n                self.launcher.creating_folder = true;\n                self.launcher.new_name.clear();\n                self.launcher.message = None;\n            }\n            KeyCode::Char('r') | KeyCode::Char('R') => self.launcher.refresh(),\n", 'folder shortcut')
marker = '        } else if self.launcher.creating {\n'
render = '''        } else if self.launcher.creating_folder {\n            push_line(&mut out, &framed_left("", inner_width, "38;5;244"));\n            push_line(&mut out, &framed_left(&format!("\\x1b[1;38;5;222m{ICON_FOLDER}\\x1b[0m   NUEVA CARPETA"), inner_width, "38;5;244"));\n            push_line(&mut out, &framed_left(&format!("\\x1b[38;5;250mNombre:\\x1b[0m {}\\x1b[?25h", self.launcher.new_name), inner_width, "38;5;244"));\n            push_line(&mut out, &framed_left("", inner_width, "38;5;244"));\n            push_line(&mut out, &framed_left("\\x1b[38;5;244mEnter crear  ·  Esc cancelar\\x1b[0m", inner_width, "38;5;244"));\n'''
s = must(s, marker, render + marker, 'folder render')
old_footer = '                    "\\x1b[38;5;244m↑↓ seleccionar  Enter abrir  N nuevo  I importar  E exportar  Z modo  Backspace subir  R refrescar\\x1b[0m",\n'
new_footer = '                    "\\x1b[38;5;244m↑↓ seleccionar  Enter abrir  N nuevo  F carpeta  P proyecto  I importar  E exportar  F11 maximizar  Backspace subir  R refrescar\\x1b[0m",\n'
if old_footer in s:
    s = s.replace(old_footer, new_footer, 1)
s = s.replace('"\\x1b[38;5;250mProyecto:\\x1b[0m {}\\x1b[?25h"', '"\\x1b[38;5;250mProyecto (nuevo o existente):\\x1b[0m {}\\x1b[?25h"', 1)
p.write_text(s, encoding='utf-8')

p = Path('src/document.rs')
s = p.read_text(encoding='utf-8')
old = '''    let mut document = read(path)?;\n    let project = project.trim().to_owned();\n    document.metadata.project = project;\n'''
new = '''    let mut document = read(path)?;\n    let old_project = document.metadata.project.clone();\n    let project = project.trim().to_owned();\n    document.metadata.project = project.clone();\n'''
s = must(s, old, new, 'project old assignment')
old = '''    write(path, &document)\n        .with_context(|| format!("No se pudo actualizar el proyecto de {}", path.display()))\n}\n'''
new = '''    write(path, &document)\n        .with_context(|| format!("No se pudo actualizar el proyecto de {}", path.display()))?;\n    if let Some(root) = path.parent() {\n        if !old_project.trim().is_empty() && !old_project.eq_ignore_ascii_case(&project) {\n            sync_project_manifest(root, &old_project)?;\n        }\n        sync_project_manifest(root, &project)?;\n    }\n    Ok(())\n}\n\nfn project_manifest_path(root: &Path, project: &str) -> PathBuf {\n    let safe = project.chars().map(|ch| if ch.is_alphanumeric() || matches!(ch, '-' | '_') { ch } else { '_' }).collect::<String>();\n    root.join(format!(".zen-project-{}.json", if safe.is_empty() { "project" } else { &safe }))\n}\n\nfn sync_project_manifest(root: &Path, project: &str) -> Result<()> {\n    let project = project.trim();\n    if project.is_empty() { return Ok(()); }\n    let documents = project_documents(root, project)?;\n    let members = documents.iter().map(|(path, metadata)| json!({\n        "path": path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),\n        "id": metadata.id,\n        "title": metadata.title,\n        "type": metadata.kind,\n        "chapter": metadata.chapter,\n        "order": metadata.order\n    })).collect::<Vec<_>>();\n    let manifest = json!({"format": 1, "type": "zen-project", "name": project, "documents": members});\n    fs::write(project_manifest_path(root, project), serde_json::to_string_pretty(&manifest)?)?;\n    Ok(())\n}\n'''
s = must(s, old, new, 'project manifest')
p.write_text(s, encoding='utf-8')
