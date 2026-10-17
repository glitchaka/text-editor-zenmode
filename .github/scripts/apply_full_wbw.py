from pathlib import Path


def repl(path, old, new):
    p = Path(path)
    text = p.read_text(encoding='utf-8')
    if old not in text:
        raise SystemExit(f'missing pattern in {path}: {old[:180]!r}')
    p.write_text(text.replace(old, new, 1), encoding='utf-8')

# Export arbitrary WBW dossier text through the existing PDF writer.
repl('src/export.rs', '''pub fn export(
    source: &Path,
    library_documents: &Path,
    exports_dir: &Path,
    format: ExportFormat,
    scope: ExportScope,
) -> Result<PathBuf> {
''', '''pub fn export_text_pdf(title: &str, body: &str, exports_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(exports_dir)?;
    let target = unique_target(exports_dir, &sanitize_filename(title), "pdf");
    let document = ExportDocument {
        title: title.to_owned(),
        body: body.to_owned(),
        page: PageProfile::default(),
        alignments: body.lines().map(|_| ParagraphAlignment::Left).collect(),
    };
    write_pdf(&target, &[document])?;
    Ok(target)
}

pub fn export(
    source: &Path,
    library_documents: &Path,
    exports_dir: &Path,
    format: ExportFormat,
    scope: ExportScope,
) -> Result<PathBuf> {
''')

# World/project library operations for empty projects and deletion.
repl('src/world.rs', '''pub fn project_path(documents: &Path, project: &str) -> PathBuf {
    documents.join(format!(".zen-project-{}.json", safe_component(project)))
}
''', '''pub fn list_projects(documents: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(documents) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|v| v.to_str()) else { continue; };
            if !file_name.starts_with(".zen-project-") || !file_name.ends_with(".json") { continue; }
            if let Ok(raw) = fs::read_to_string(&path)
                && let Ok(root) = serde_json::from_str::<Value>(&raw)
                && let Some(name) = root.get("name").and_then(Value::as_str)
                && !name.trim().is_empty()
            {
                names.push(name.to_owned());
            }
        }
    }
    for (name, _) in document::project_counts(documents) {
        if !names.iter().any(|existing| existing == &name) {
            names.push(name);
        }
    }
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names
}

pub fn delete_project(documents: &Path, project: &str) -> Result<usize> {
    let mut removed = 0usize;
    let workspace_path = project_path(documents, project);
    if workspace_path.exists() {
        fs::remove_file(&workspace_path)?;
        removed += 1;
    }
    if let Ok(entries) = fs::read_dir(documents) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !document::is_native_path(&path) { continue; }
            let Some(metadata) = document::read_metadata(&path) else { continue; };
            if metadata.project.trim() != project.trim() { continue; }
            let sidecar = path.with_file_name(format!("{}.zen.json", path.file_name().and_then(|v| v.to_str()).unwrap_or("document.txt")));
            if fs::remove_file(&path).is_ok() { removed += 1; }
            let _ = fs::remove_file(sidecar);
        }
    }
    Ok(removed)
}

pub fn project_path(documents: &Path, project: &str) -> PathBuf {
    documents.join(format!(".zen-project-{}.json", safe_component(project)))
}
''')

# App imports and launcher state.
repl('src/app.rs', '''    library::LibraryPaths,
    world::{self, WorldSection},
};
''', '''    library::LibraryPaths,
    wbw_tools,
    world::{self, WorldSection},
};
''')
repl('src/app.rs', '''    workspace_edit_field: Option<String>,
    message: Option<String>,
''', '''    workspace_edit_field: Option<String>,
    workspace_visual: bool,
    workspace_visual_zoom: i32,
    workspace_visual_x: i32,
    workspace_visual_y: i32,
    workspace_create_project: bool,
    workspace_project_picker: bool,
    workspace_project_picker_row: usize,
    workspace_delete_confirm: bool,
    message: Option<String>,
''')
repl('src/app.rs', '''            workspace_edit_field: None,
            message: None,
''', '''            workspace_edit_field: None,
            workspace_visual: false,
            workspace_visual_zoom: 1,
            workspace_visual_x: 0,
            workspace_visual_y: 0,
            workspace_create_project: false,
            workspace_project_picker: false,
            workspace_project_picker_row: 0,
            workspace_delete_confirm: false,
            message: None,
''')

# Insert empty-project creator and project library picker before normal WBW workspace handling.
needle = '''    fn launcher_key(&mut self, key: KeyEvent) {
        if let Some(project) = self.launcher.workspace_project.clone() {
'''
insert = '''    fn launcher_key(&mut self, key: KeyEvent) {
        if self.launcher.workspace_create_project {
            match key.code {
                KeyCode::Esc => { self.launcher.workspace_create_project = false; self.launcher.workspace_input.clear(); }
                KeyCode::Enter => {
                    let name = self.launcher.workspace_input.trim().to_owned();
                    if name.is_empty() {
                        self.launcher.message = Some("Escribe el nombre de la obra.".into());
                    } else {
                        match world::ProjectWorkspace::load_or_create(&self.library.documents, &name) {
                            Ok(_) => {
                                self.launcher.workspace_project = Some(name);
                                self.launcher.workspace_create_project = false;
                                self.launcher.workspace_project_picker = false;
                                self.launcher.workspace_input.clear();
                                self.launcher.workspace_selected = 0;
                                self.launcher.message = Some("Obra creada.".into());
                            }
                            Err(error) => self.launcher.message = Some(format!("No se pudo crear la obra: {error}")),
                        }
                    }
                }
                KeyCode::Backspace => { self.launcher.workspace_input.pop(); }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => self.launcher.workspace_input.push(ch),
                _ => {}
            }
            self.render_launcher();
            return;
        }

        if self.launcher.workspace_project_picker {
            let projects = world::list_projects(&self.library.documents);
            match key.code {
                KeyCode::Esc => self.launcher.workspace_project_picker = false,
                KeyCode::Up => self.launcher.workspace_project_picker_row = self.launcher.workspace_project_picker_row.saturating_sub(1),
                KeyCode::Down => self.launcher.workspace_project_picker_row = (self.launcher.workspace_project_picker_row + 1).min(projects.len().saturating_sub(1)),
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.launcher.workspace_project_picker = false;
                    self.launcher.workspace_create_project = true;
                    self.launcher.workspace_input.clear();
                }
                KeyCode::Enter => {
                    if let Some(project) = projects.get(self.launcher.workspace_project_picker_row).cloned() {
                        self.launcher.workspace_project = Some(project);
                        self.launcher.workspace_project_picker = false;
                        self.launcher.workspace_selected = 0;
                        self.launcher.workspace_detail = false;
                        self.launcher.workspace_row = 0;
                    }
                }
                _ => {}
            }
            self.render_launcher();
            return;
        }

        if let Some(project) = self.launcher.workspace_project.clone() {
'''
repl('src/app.rs', needle, insert)

# Visual-board and destructive confirmation handlers at top of workspace block.
repl('src/app.rs', '''            let section = WorldSection::ALL[self.launcher.workspace_selected.min(WorldSection::ALL.len() - 1)];

            if let Some(field) = self.launcher.workspace_edit_field.clone() {
''', '''            let section = WorldSection::ALL[self.launcher.workspace_selected.min(WorldSection::ALL.len() - 1)];

            if self.launcher.workspace_delete_confirm {
                match key.code {
                    KeyCode::Esc => self.launcher.workspace_delete_confirm = false,
                    KeyCode::Enter => {
                        match world::delete_project(&self.library.documents, &project) {
                            Ok(count) => {
                                self.launcher.workspace_project = None;
                                self.launcher.workspace_delete_confirm = false;
                                self.launcher.refresh();
                                self.launcher.message = Some(format!("Obra eliminada · {count} archivo(s) retirado(s)."));
                            }
                            Err(error) => self.launcher.message = Some(format!("No se pudo eliminar la obra: {error}")),
                        }
                    }
                    _ => {}
                }
                self.render_launcher();
                return;
            }

            if self.launcher.workspace_visual {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('v') | KeyCode::Char('V') => self.launcher.workspace_visual = false,
                    KeyCode::Char('+') | KeyCode::Char('=') => self.launcher.workspace_visual_zoom = (self.launcher.workspace_visual_zoom - 1).max(1),
                    KeyCode::Char('-') => self.launcher.workspace_visual_zoom = (self.launcher.workspace_visual_zoom + 1).min(8),
                    KeyCode::Left => self.launcher.workspace_visual_x -= 2,
                    KeyCode::Right => self.launcher.workspace_visual_x += 2,
                    KeyCode::Up => self.launcher.workspace_visual_y -= 1,
                    KeyCode::Down => self.launcher.workspace_visual_y += 1,
                    _ => {}
                }
                self.render_launcher();
                return;
            }

            if let Some(field) = self.launcher.workspace_edit_field.clone() {
''')

# Add complete workspace actions before Enter handler.
anchor = '''                KeyCode::Char('i') | KeyCode::Char('I') if !self.launcher.workspace_detail => {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Worldbuilder Writer", &["wbw"]).pick_file() {
                        match world::ProjectWorkspace::import_package(&path, &self.library.documents) {
                            Ok(workspace) => {
                                self.launcher.workspace_project = Some(workspace.name);
                                self.launcher.message = Some("Paquete WBW importado.".into());
                            }
                            Err(error) => self.launcher.message = Some(format!("No se pudo importar WBW: {error}")),
                        }
                    }
                }
                KeyCode::Enter => {
'''
replacement = '''                KeyCode::Char('i') | KeyCode::Char('I') if !self.launcher.workspace_detail => {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Worldbuilder Writer", &["wbw"]).pick_file() {
                        match wbw_tools::validate_package(&path).and_then(|_| world::ProjectWorkspace::import_package(&path, &self.library.documents)) {
                            Ok(workspace) => {
                                self.launcher.workspace_project = Some(workspace.name);
                                self.launcher.message = Some("Paquete WBW validado e importado.".into());
                            }
                            Err(error) => self.launcher.message = Some(format!("No se pudo importar WBW: {error}")),
                        }
                    }
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    match world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                        .and_then(|workspace| wbw_tools::export_dossier_pdf(&workspace, &self.library.exports, if self.launcher.workspace_detail { Some(section) } else { None }))
                    {
                        Ok(path) => self.launcher.message = Some(format!("PDF WBW: {}", path.display())),
                        Err(error) => self.launcher.message = Some(format!("No se pudo exportar PDF: {error}")),
                    }
                }
                KeyCode::Char('k') | KeyCode::Char('K') if !self.launcher.workspace_detail => {
                    if let Some(directory) = rfd::FileDialog::new().pick_folder() {
                        match world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                            .and_then(|workspace| wbw_tools::backup_to_directory(&workspace, &self.library.documents, &directory))
                        {
                            Ok(path) => self.launcher.message = Some(format!("Copia WBW: {}", path.display())),
                            Err(error) => self.launcher.message = Some(format!("No se pudo crear copia: {error}")),
                        }
                    }
                }
                KeyCode::Char('x') | KeyCode::Char('X') if !self.launcher.workspace_detail => {
                    self.launcher.workspace_delete_confirm = true;
                }
                KeyCode::Char('v') | KeyCode::Char('V') if self.launcher.workspace_detail && matches!(section, WorldSection::Characters | WorldSection::Relationships | WorldSection::Maps) => {
                    self.launcher.workspace_visual = true;
                    self.launcher.workspace_visual_zoom = 1;
                    self.launcher.workspace_visual_x = 0;
                    self.launcher.workspace_visual_y = 0;
                }
                KeyCode::Char('t') | KeyCode::Char('T') if section == WorldSection::Review => {
                    match world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                        .and_then(|mut workspace| wbw_tools::review_project(&mut workspace))
                    {
                        Ok(count) => self.launcher.message = Some(format!("Revisión terminada · {count} hallazgo(s).")),
                        Err(error) => self.launcher.message = Some(format!("No se pudo revisar: {error}")),
                    }
                }
                KeyCode::Char('o') | KeyCode::Char('O') if self.launcher.workspace_detail && matches!(section, WorldSection::Media | WorldSection::References) => {
                    if let Ok(workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        let target = if section == WorldSection::Media {
                            wbw_tools::media_target(&workspace, self.launcher.workspace_row)
                        } else {
                            wbw_tools::reference_target(&workspace, self.launcher.workspace_row)
                        };
                        if let Some(target) = target {
                            match wbw_tools::open_external(&target) {
                                Ok(()) => self.launcher.message = Some(format!("Abierto: {target}")),
                                Err(error) => self.launcher.message = Some(format!("No se pudo abrir: {error}")),
                            }
                        } else {
                            self.launcher.message = Some("El registro no tiene ruta o URL.".into());
                        }
                    }
                }
                KeyCode::Enter => {
'''
repl('src/app.rs', anchor, replacement)

# W now opens a real project library; J creates an empty work immediately.
repl('src/app.rs', '''            KeyCode::Char('w') | KeyCode::Char('W') => {
                let selected_project = self.launcher.selected_entry()
                    .and_then(|entry| entry.metadata.as_ref())
                    .map(|metadata| metadata.project.trim().to_owned())
                    .filter(|project| !project.is_empty());
                let fallback = document::project_counts(&self.library.documents).into_iter().next().map(|(name, _)| name);
                if let Some(project) = selected_project.or(fallback) {
                    match world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        Ok(_) => {
                            self.launcher.workspace_project = Some(project);
                            self.launcher.workspace_selected = 0;
                            self.launcher.workspace_detail = false;
                            self.launcher.workspace_row = 0;
                            self.launcher.message = None;
                        }
                        Err(error) => self.launcher.message = Some(format!("No se pudo abrir WBW: {error}")),
                    }
                } else {
                    self.launcher.message = Some("Asigna primero un documento a un proyecto con P.".into());
                }
            }
''', '''            KeyCode::Char('w') | KeyCode::Char('W') => {
                let selected_project = self.launcher.selected_entry()
                    .and_then(|entry| entry.metadata.as_ref())
                    .map(|metadata| metadata.project.trim().to_owned())
                    .filter(|project| !project.is_empty());
                if let Some(project) = selected_project {
                    self.launcher.workspace_project = Some(project);
                    self.launcher.workspace_selected = 0;
                    self.launcher.workspace_detail = false;
                    self.launcher.workspace_row = 0;
                    self.launcher.message = None;
                } else {
                    self.launcher.workspace_project_picker = true;
                    self.launcher.workspace_project_picker_row = 0;
                    if world::list_projects(&self.library.documents).is_empty() {
                        self.launcher.workspace_project_picker = false;
                        self.launcher.workspace_create_project = true;
                        self.launcher.workspace_input.clear();
                    }
                }
            }
            KeyCode::Char('j') | KeyCode::Char('J') => {
                self.launcher.workspace_create_project = true;
                self.launcher.workspace_input.clear();
                self.launcher.message = None;
            }
''')

# Render project picker/create/delete/visual modes before ordinary file list.
repl('src/app.rs', '''        if let Some(project) = self.launcher.workspace_project.as_ref() {
            match world::ProjectWorkspace::load_or_create(&self.library.documents, project) {
''', '''        if self.launcher.workspace_create_project {
            push_line(&mut out, &framed_left("\\x1b[1;38;5;222mNUEVA OBRA WBW\\x1b[0m", inner_width, "38;5;244"));
            push_line(&mut out, &framed_left(&format!("Nombre: {}\\x1b[?25h", self.launcher.workspace_input), inner_width, "38;5;244"));
            push_line(&mut out, &framed_left("Enter crear · Esc cancelar", inner_width, "38;5;244"));
        } else if self.launcher.workspace_project_picker {
            push_line(&mut out, &framed_left("\\x1b[1;38;5;222mBIBLIOTECA WBW\\x1b[0m", inner_width, "38;5;244"));
            let projects = world::list_projects(&self.library.documents);
            if projects.is_empty() {
                push_line(&mut out, &framed_left("(sin obras)", inner_width, "38;5;244"));
            } else {
                for (index, project) in projects.iter().enumerate().take(14) {
                    let marker = if index == self.launcher.workspace_project_picker_row { "▶" } else { " " };
                    push_line(&mut out, &framed_left(&truncate(&format!("{marker} {project}"), inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                }
            }
            push_line(&mut out, &framed_left("↑↓ seleccionar · Enter abrir · N nueva obra · Esc volver", inner_width, "38;5;244"));
        } else if let Some(project) = self.launcher.workspace_project.as_ref() {
            match world::ProjectWorkspace::load_or_create(&self.library.documents, project) {
''')

# Workspace render visual/confirm states.
repl('src/app.rs', '''                    if let Some(field) = self.launcher.workspace_edit_field.as_ref() {
''', '''                    if self.launcher.workspace_delete_confirm {
                        push_line(&mut out, &framed_left("\\x1b[1;38;5;203mELIMINAR OBRA COMPLETA\\x1b[0m", inner_width, "38;5;244"));
                        push_line(&mut out, &framed_left(&format!("Se eliminarán el proyecto y sus documentos: {project}"), inner_width, "38;5;244"));
                        push_line(&mut out, &framed_left("Enter CONFIRMAR · Esc cancelar", inner_width, "38;5;244"));
                    } else if self.launcher.workspace_visual {
                        let section = WorldSection::ALL[self.launcher.workspace_selected];
                        push_line(&mut out, &framed_left(&format!("\\x1b[1;38;5;222m{} · VISTA\\x1b[0m", section.label()), inner_width, "38;5;244"));
                        let lines = if section == WorldSection::Maps {
                            wbw_tools::map_preview(&workspace, self.launcher.workspace_row, inner_width.saturating_sub(2), self.launcher.workspace_visual_zoom, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y)
                        } else {
                            wbw_tools::relationship_board(&workspace, inner_width.saturating_sub(2), self.launcher.workspace_visual_zoom, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y)
                        };
                        for line in lines.into_iter().take(22) {
                            push_line(&mut out, &framed_left(&truncate(&line, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                        }
                        push_line(&mut out, &framed_left("Flechas mover · +/- zoom · V/Esc volver", inner_width, "38;5;244"));
                    } else if let Some(field) = self.launcher.workspace_edit_field.as_ref() {
''')

# More truthful controls in workspace footer.
repl('src/app.rs', '''                            push_line(&mut out, &framed_left(&format!("N nuevo · Enter ficha · R renombrar · D eliminar · Ctrl+↑↓ mover{extra} · Esc volver"), inner_width, "38;5;244"));
''', '''                            let visual = if matches!(section, WorldSection::Characters | WorldSection::Relationships | WorldSection::Maps) { " · V vista" } else { "" };
                            let review = if section == WorldSection::Review { " · T revisar" } else { "" };
                            let open = if matches!(section, WorldSection::Media | WorldSection::References) { " · O abrir" } else { "" };
                            push_line(&mut out, &framed_left(&format!("N nuevo · Enter ficha · R renombrar · D eliminar · Ctrl+↑↓ mover{extra}{visual}{review}{open} · P PDF · Esc volver"), inner_width, "38;5;244"));
''')
repl('src/app.rs', '''                        push_line(&mut out, &framed_left("↑↓ seleccionar · Enter abrir · B exportar .wbw · I importar .wbw · Esc archivos", inner_width, "38;5;244"));
''', '''                        push_line(&mut out, &framed_left("↑↓ seleccionar · Enter abrir · B .wbw · I importar · P PDF · K copia/Drive · X eliminar · Esc archivos", inner_width, "38;5;244"));
''')

# Launcher help exposes WBW library and empty work creation.
text_path = Path('src/app.rs')
text = text_path.read_text(encoding='utf-8')
text = text.replace('P proyecto  W WBW  I importar', 'P proyecto  W biblioteca WBW  J nueva obra  I importar')
text_path.write_text(text, encoding='utf-8')

# README: one concise operation section.
p = Path('README.md')
text = p.read_text(encoding='utf-8')
section = '''\n\n### WBW — flujo completo integrado\n\n`W` abre la biblioteca de obras WBW y `J` crea una obra vacía. Dentro de una obra se conservan Proyecto, Manuscrito, Escenas, Personajes, Relaciones, Hilos/teorías, Cronología, Atlas, Mapas, Magia, Referencias, Documentación/notas, Revisión, Maquetación, Multimedia, Configuración y Estadísticas. `V` abre las vistas de tablero/red/mapa, `T` ejecuta revisión automática, `O` abre medios y referencias, `P` exporta PDF por sección o dossier completo, `B` genera el paquete `.wbw`, `I` lo valida e importa, `K` crea una copia en cualquier carpeta local/sincronizada (incluido Google Drive Desktop) y `X` elimina una obra completa con confirmación.\n'''
if '### WBW — flujo completo integrado' not in text:
    text += section
    p.write_text(text, encoding='utf-8')

# CHANGELOG
p = Path('CHANGELOG.md')
text = p.read_text(encoding='utf-8')
entry = '''## WBW functional completion — 2026-10-17\n\n- Biblioteca WBW real con obras vacías, apertura y eliminación confirmada.\n- Vistas navegables de tablero de personajes/relaciones y mapas con pan/zoom.\n- Revisión automática de repeticiones, muletillas y frecuencias, guardada como hallazgos editables.\n- Apertura de medios locales/URLs y referencias externas.\n- PDF por sección y dossier completo; copia portable `.wbw` a cualquier carpeta, incluida una carpeta sincronizada con Drive.\n- Validación de paquetes contra rutas inseguras, recursos gigantes y expansión excesiva antes de importar.\n\n'''
if '## WBW functional completion — 2026-10-17' not in text:
    marker = text.find('## ')
    text = text[:marker] + entry + text[marker:] if marker >= 0 else entry + text
    p.write_text(text, encoding='utf-8')
