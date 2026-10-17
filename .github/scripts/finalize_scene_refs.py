from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    text = p.read_text(encoding='utf-8')
    if old not in text:
        raise SystemExit(f'missing pattern in {path}: {old[:180]!r}')
    p.write_text(text.replace(old, new, 1), encoding='utf-8')

# Persist the documents root in each workspace so scene records can own real TXT documents.
replace_once('src/world.rs', '''pub struct ProjectWorkspace {
    pub name: String,
    pub path: PathBuf,
    root: Value,
}
''', '''pub struct ProjectWorkspace {
    pub name: String,
    pub path: PathBuf,
    documents: PathBuf,
    root: Value,
}
''')
replace_once('src/world.rs', '''        let mut workspace = Self {
            name: project.to_owned(),
            path,
            root,
        };
''', '''        let mut workspace = Self {
            name: project.to_owned(),
            path,
            documents: documents.to_path_buf(),
            root,
        };
''')
replace_once('src/world.rs', '''        let mut workspace = Self {
            name: name.clone(),
            path: project_path(documents, &name),
            root,
        };
''', '''        let mut workspace = Self {
            name: name.clone(),
            path: project_path(documents, &name),
            documents: documents.to_path_buf(),
            root,
        };
''')

# Real native TXT for scenes, with stable path and metadata.
replace_once('src/world.rs', '''    pub fn add_record(&mut self, section: WorldSection, title: &str) -> Result<()> {
        if !section.is_record_list() {
            return Ok(());
        }
        let record = match section {
            WorldSection::Scenes => json!({
                "id": generate_id(), "title": title, "chapter": null, "pov": "", "location": "",
                "timeline": "", "narrative_layer": "", "status": "draft", "summary": "", "words": 0, "order": 0
            }),
''', '''    pub fn add_record(&mut self, section: WorldSection, title: &str) -> Result<()> {
        if !section.is_record_list() {
            return Ok(());
        }
        let record = match section {
            WorldSection::Scenes => {
                let id = generate_id();
                let order = self.array(WorldSection::Scenes).len() as i64 * 10 + 10;
                let path = unique_scene_path(&self.documents, &self.name, title);
                let document = document::HsstDocument {
                    metadata: document::DocumentMetadata {
                        format: document::FORMAT_VERSION,
                        id: id.clone(),
                        title: title.to_owned(),
                        project: self.name.clone(),
                        kind: "scene".into(),
                        chapter: None,
                        order,
                        language: "es-CL".into(),
                        status: "draft".into(),
                        page: crate::page::PageProfile::default(),
                    },
                    body: String::new(),
                    formatting: json!({"version": 1, "unit": "utf8-byte", "runs": [], "paragraphs": []}),
                };
                document::write(&path, &document)?;
                json!({
                    "id": id, "title": title, "path": path.to_string_lossy(), "chapter": null,
                    "pov": "", "location": "", "timeline": "", "narrative_layer": "",
                    "status": "draft", "summary": "", "words": 0, "order": order
                })
            },
''')

# Keep scene metadata synchronized with the underlying TXT sidecar.
replace_once('src/world.rs', '''        object.insert(key.to_owned(), value);
        self.touch();
        self.save()?;
        Ok(true)
    }
''', '''        object.insert(key.to_owned(), value);
        if section == WorldSection::Scenes {
            self.sync_scene_document(index)?;
        }
        self.touch();
        self.save()?;
        Ok(true)
    }
''')
replace_once('src/world.rs', '''        rows.remove(index);
        self.touch();
        self.save()?;
        Ok(true)
''', '''        let removed = rows.remove(index);
        if section == WorldSection::Scenes {
            if let Some(path) = removed.get("path").and_then(Value::as_str).map(PathBuf::from) {
                let _ = fs::remove_file(&path);
                let sidecar = path.with_file_name(format!("{}.zen.json", path.file_name().and_then(|v| v.to_str()).unwrap_or("scene.txt")));
                let _ = fs::remove_file(sidecar);
            }
        }
        self.touch();
        self.save()?;
        Ok(true)
''')
replace_once('src/world.rs', '''        if target != index {
            let value = rows.remove(index);
            rows.insert(target, value);
            for (order, row) in rows.iter_mut().enumerate() {
                if let Some(object) = row.as_object_mut() {
                    object.insert("order".into(), json!(order as i64));
                }
            }
            self.touch();
            self.save()?;
        }
''', '''        if target != index {
            let value = rows.remove(index);
            rows.insert(target, value);
            for (order, row) in rows.iter_mut().enumerate() {
                if let Some(object) = row.as_object_mut() {
                    object.insert("order".into(), json!((order as i64 + 1) * 10));
                }
            }
            if section == WorldSection::Scenes {
                for scene_index in 0..rows.len() {
                    self.sync_scene_document(scene_index)?;
                }
            }
            self.touch();
            self.save()?;
        }
''')

# Insert helpers before sync_manuscript and exclude scene documents from manuscript list.
replace_once('src/world.rs', '''    fn sync_manuscript(&mut self, documents: &Path) -> Result<()> {
''', '''    fn sync_scene_document(&mut self, index: usize) -> Result<()> {
        let Some(scene) = self.array(WorldSection::Scenes).get(index) else { return Ok(()); };
        let Some(path) = scene.get("path").and_then(Value::as_str).map(PathBuf::from) else { return Ok(()); };
        if !path.exists() { return Ok(()); }
        let mut doc = document::read(&path)?;
        doc.metadata.project = self.name.clone();
        doc.metadata.kind = "scene".into();
        doc.metadata.title = scene.get("title").and_then(Value::as_str).unwrap_or(&doc.metadata.title).to_owned();
        doc.metadata.status = scene.get("status").and_then(Value::as_str).unwrap_or(&doc.metadata.status).to_owned();
        doc.metadata.order = scene.get("order").and_then(Value::as_i64).unwrap_or(doc.metadata.order);
        doc.metadata.chapter = scene.get("chapter").and_then(Value::as_u64).and_then(|value| u32::try_from(value).ok()).filter(|value| *value > 0);
        document::write(&path, &doc)?;
        Ok(())
    }

    fn sync_scene_records(&mut self) -> Result<()> {
        let mut replacements = Vec::new();
        for (index, scene) in self.array(WorldSection::Scenes).iter().enumerate() {
            let has_path = scene.get("path").and_then(Value::as_str).map(Path::new).is_some_and(Path::exists);
            if has_path { continue; }
            let title = scene.get("title").and_then(Value::as_str).unwrap_or("Escena").to_owned();
            let path = unique_scene_path(&self.documents, &self.name, &title);
            let id = scene.get("id").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(generate_id);
            let order = scene.get("order").and_then(Value::as_i64).unwrap_or((index as i64 + 1) * 10);
            let chapter = scene.get("chapter").and_then(Value::as_u64).and_then(|value| u32::try_from(value).ok()).filter(|value| *value > 0);
            let status = scene.get("status").and_then(Value::as_str).unwrap_or("draft").to_owned();
            let doc = document::HsstDocument {
                metadata: document::DocumentMetadata {
                    format: document::FORMAT_VERSION, id, title: title.clone(), project: self.name.clone(), kind: "scene".into(), chapter, order,
                    language: "es-CL".into(), status, page: crate::page::PageProfile::default(),
                },
                body: String::new(), formatting: json!({"version":1,"unit":"utf8-byte","runs":[],"paragraphs":[]}),
            };
            document::write(&path, &doc)?;
            replacements.push((index, path));
        }
        for (index, path) in replacements {
            if let Some(object) = self.array_mut(WorldSection::Scenes).get_mut(index).and_then(Value::as_object_mut) {
                object.insert("path".into(), json!(path.to_string_lossy()));
            }
        }
        Ok(())
    }

    fn sync_manuscript(&mut self, documents: &Path) -> Result<()> {
''')
replace_once('src/world.rs', '''            if metadata.project.trim() != self.name {
                continue;
            }
''', '''            if metadata.project.trim() != self.name || metadata.kind.eq_ignore_ascii_case("scene") {
                continue;
            }
''')
replace_once('src/world.rs', '''        workspace.normalize();
        workspace.sync_manuscript(documents)?;
''', '''        workspace.normalize();
        workspace.sync_scene_records()?;
        workspace.sync_manuscript(documents)?;
''')
# same sequence appears in import_package; second occurrence
replace_once('src/world.rs', '''        workspace.normalize();
        workspace.sync_manuscript(documents)?;
''', '''        workspace.normalize();
        workspace.sync_scene_records()?;
        workspace.sync_manuscript(documents)?;
''')

# helper unique path before project_path
replace_once('src/world.rs', '''pub fn project_path(documents: &Path, project: &str) -> PathBuf {
''', '''fn unique_scene_path(documents: &Path, project: &str, title: &str) -> PathBuf {
    let base = format!("{} - {}", safe_component(project), safe_component(title));
    let mut path = documents.join(format!("{base}.txt"));
    for index in 2..10_000 {
        if !path.exists() { break; }
        path = documents.join(format!("{base} ({index}).txt"));
    }
    path
}

pub fn project_path(documents: &Path, project: &str) -> PathBuf {
''')

# Semantic references: F3 inserts a stable inline [[ref:<id>|<label>]] marker from a project reference picker.
replace_once('src/app.rs', '''    closed_tabs: Vec<PathBuf>,
}
''', '''    closed_tabs: Vec<PathBuf>,
    reference_picker: Option<(String, Vec<(String, String)>, usize)>,
}
''')
replace_once('src/app.rs', '''            closed_tabs: Vec::new(),
        };
''', '''            closed_tabs: Vec::new(),
            reference_picker: None,
        };
''')

# Render picker as transient terminal overlay using existing parser, and handle keys before normal editor input.
replace_once('src/app.rs', '''    fn editor_key(&mut self, key: KeyEvent) {
        // F2 is a transient Helix code-action menu. Once the user confirms an
''', '''    fn editor_key(&mut self, key: KeyEvent) {
        if let Some((project, rows, selected)) = self.reference_picker.clone() {
            match key.code {
                KeyCode::Esc => { self.reference_picker = None; self.reset_parser(); if let Some(editor) = self.editor.as_mut() { let _ = editor.force_terminal_redraw(); } }
                KeyCode::Up => { self.reference_picker = Some((project, rows, selected.saturating_sub(1))); }
                KeyCode::Down => { self.reference_picker = Some((project, rows.clone(), (selected + 1).min(rows.len().saturating_sub(1)))); }
                KeyCode::Enter => {
                    if let Some((id, label)) = rows.get(selected) {
                        let marker = format!("[[ref:{id}|{label}]]");
                        self.reference_picker = None;
                        self.insert_symbol(&marker);
                    }
                }
                _ => {}
            }
            self.dirty = true;
            return;
        }
        // F2 is a transient Helix code-action menu. Once the user confirms an
''')
replace_once('src/app.rs', '''        if key.code == KeyCode::F(11) && key.modifiers.is_empty() {
''', '''        if key.code == KeyCode::F(3) && key.modifiers.is_empty() {
            if let Some(current) = self.current_file.as_ref().and_then(|path| document::read_metadata(path)) {
                if !current.project.trim().is_empty() {
                    if let Ok(workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &current.project) {
                        let refs = workspace.semantic_references();
                        if refs.is_empty() {
                            self.launcher.message = Some("El proyecto no tiene referencias semánticas.".into());
                        } else {
                            self.reference_picker = Some((current.project, refs, 0));
                            self.render_reference_picker();
                        }
                    }
                }
            }
            return;
        }
        if key.code == KeyCode::F(11) && key.modifiers.is_empty() {
''')
# add render helper before terminal cell function
replace_once('src/app.rs', '''    fn terminal_cell_at(&self, x: f32, y: f32) -> Option<(u16, u16)> {
''', '''    fn render_reference_picker(&mut self) {
        let Some((_, rows, selected)) = self.reference_picker.as_ref() else { return; };
        self.reset_parser();
        let (cols, terminal_rows) = self.terminal_size();
        let width = cols as usize;
        let frame_width = width.saturating_sub(10).clamp(44, 84);
        let inner = frame_width.saturating_sub(2);
        let mut out = String::from("\\x1b[2J\\x1b[H\\x1b[?25l");
        push_line(&mut out, &format!("\\x1b[38;5;244m╭{}╮\\x1b[0m", "─".repeat(inner)));
        push_line(&mut out, &framed_left("\\x1b[1;38;5;222mREFERENCIA SEMÁNTICA\\x1b[0m", inner, "38;5;244"));
        for (index, (_, label)) in rows.iter().enumerate().take((terminal_rows as usize).saturating_sub(5).min(16)) {
            let marker = if index == *selected { "▶" } else { " " };
            push_line(&mut out, &framed_left(&truncate(&format!("{marker} {label}"), inner.saturating_sub(2)), inner, "38;5;244"));
        }
        push_line(&mut out, &framed_left("↑↓ seleccionar · Enter insertar · Esc cancelar", inner, "38;5;244"));
        push_last_line(&mut out, &format!("\\x1b[38;5;244m╰{}╯\\x1b[0m", "─".repeat(inner)));
        self.parser.process(out.as_bytes());
        self.dirty = true;
    }

    fn terminal_cell_at(&self, x: f32, y: f32) -> Option<(u16, u16)> {
''')

# Expose reference list from world model.
replace_once('src/world.rs', '''    pub fn rows(&self, section: WorldSection) -> Vec<WorldRow> {
''', '''    pub fn semantic_references(&self) -> Vec<(String, String)> {
        let mut refs = Vec::new();
        for section in [WorldSection::Characters, WorldSection::World, WorldSection::Magic, WorldSection::References, WorldSection::Notes] {
            for item in self.array(section) {
                let Some(id) = item.get("id").and_then(Value::as_str) else { continue; };
                let label = item.get("name").or_else(|| item.get("title")).and_then(Value::as_str).unwrap_or("Referencia");
                refs.push((id.to_owned(), format!("{} · {}", section.label(), label)));
            }
        }
        refs
    }

    pub fn rows(&self, section: WorldSection) -> Vec<WorldRow> {
''')

# Update visible editor help to document F3 semantic reference action.
replace_once('src/editor.rs', 'F2: acciones · F4: Pomodoro · F11: pantalla completa', 'F2: acciones · F3: referencia · F4: Pomodoro · F11: pantalla completa')

# Changelog
p = Path('CHANGELOG.md')
log = p.read_text(encoding='utf-8')
entry = '''## Scene documents and semantic references — 2026-10-17\n\n- Las escenas WBW son documentos TXT nativos reales, se abren en el editor/pestañas y sincronizan título, capítulo, estado y orden con su ficha.\n- Las escenas heredadas sin archivo se migran automáticamente a TXT al abrir el proyecto.\n- Manuscrito y escenas quedan separados: el índice de manuscrito no duplica documentos de tipo escena.\n- F3 abre el selector de referencias semánticas del proyecto e inserta un marcador estable enlazado por ID a personajes, Atlas, magia, referencias o documentación.\n\n'''
if '## Scene documents and semantic references — 2026-10-17' not in log:
    marker = log.find('## ')
    log = log[:marker] + entry + log[marker:] if marker >= 0 else entry + log
    p.write_text(log, encoding='utf-8')
