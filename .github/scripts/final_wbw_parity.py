from pathlib import Path


def repl(path, old, new):
    p = Path(path)
    text = p.read_text(encoding='utf-8')
    if old not in text:
        raise SystemExit(f'missing pattern in {path}: {old[:200]!r}')
    p.write_text(text.replace(old, new, 1), encoding='utf-8')

# ---------------- world.rs ----------------
# Import manuscript preserving the native structured formatting produced by document importer.
repl('src/world.rs', '''    pub fn create_chapter(&mut self, title: &str) -> Result<PathBuf> {
''', '''    pub fn import_chapter(&mut self, source: &Path) -> Result<PathBuf> {
        let path = document::import_to_native(source, &self.documents)?;
        let index = self.array(WorldSection::Manuscript).len();
        let mut doc = document::read(&path)?;
        doc.metadata.project = self.name.clone();
        doc.metadata.kind = "chapter".into();
        doc.metadata.chapter = Some(index as u32 + 1);
        doc.metadata.order = (index as i64 + 1) * 10;
        doc.metadata.status = "draft".into();
        document::write(&path, &doc)?;
        self.sync_manuscript(&self.documents.clone())?;
        self.touch();
        self.save()?;
        Ok(path)
    }

    pub fn setting_list(&self, key: &str) -> Vec<String> {
        self.root.get("settings")
            .and_then(|value| value.get(key))
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default()
    }

    pub fn character_id(&self, index: usize) -> Option<String> {
        self.array(WorldSection::Characters).get(index)?.get("id")?.as_str().map(str::to_owned)
    }

    pub fn toggle_character_visibility(&mut self, index: usize) -> Result<bool> {
        let Some(character) = self.array_mut(WorldSection::Characters).get_mut(index).and_then(Value::as_object_mut) else { return Ok(false); };
        let visible = character.get("visible").and_then(Value::as_bool).unwrap_or(true);
        character.insert("visible".into(), json!(!visible));
        self.touch();
        self.save()?;
        Ok(!visible)
    }

    pub fn create_chapter(&mut self, title: &str) -> Result<PathBuf> {
''')

# Character visibility field.
repl('src/world.rs', '''                "background": "", "physical": "", "traits": [], "evidence": [], "chapters": [], "color": "", "x": 0, "y": 0
''', '''                "background": "", "physical": "", "traits": [], "evidence": [], "chapters": [], "color": "", "visible": true, "x": 0, "y": 0
''')

# Scene file mtime becomes updated timestamp while word count synchronizes.
repl('src/world.rs', '''            let words = document::read(&path).map(|doc| doc.body.split_whitespace().count()).unwrap_or(0);
            Some((index, words))
        }).collect::<Vec<_>>();
        for (index, words) in snapshots {
            if let Some(object) = self.array_mut(WorldSection::Scenes).get_mut(index).and_then(Value::as_object_mut) {
                object.insert("words".into(), json!(words));
            }
''', '''            let words = document::read(&path).map(|doc| doc.body.split_whitespace().count()).unwrap_or(0);
            let updated = fs::metadata(&path).and_then(|meta| meta.modified()).ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map(|duration| duration.as_secs()).unwrap_or_else(unix_seconds);
            Some((index, words, updated))
        }).collect::<Vec<_>>();
        for (index, words, updated) in snapshots {
            if let Some(object) = self.array_mut(WorldSection::Scenes).get_mut(index).and_then(Value::as_object_mut) {
                object.insert("words".into(), json!(words));
                object.insert("updated".into(), json!(updated));
            }
''')

# Portable arbitrary attached resources: cover/banner/icon/character image/map background/attachments.
# Add recursive helpers before safe_component.
repl('src/world.rs', '''fn safe_component(value: &str) -> String {
''', '''fn portableize_resources(value: &mut Value, files: &mut Vec<(String, PathBuf)>, counter: &mut usize) {
    match value {
        Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if key == "path" || key == "package_path" { continue; }
                portableize_resources(child, files, counter);
            }
        }
        Value::Array(values) => {
            for child in values { portableize_resources(child, files, counter); }
        }
        Value::String(text) => {
            let path = PathBuf::from(text.as_str());
            if path.is_file() {
                *counter += 1;
                let name = path.file_name().and_then(|value| value.to_str()).map(safe_component).unwrap_or_else(|| format!("resource-{}", counter));
                let package_path = format!("resources/{:04}-{name}", counter);
                files.push((package_path.clone(), path));
                *text = format!("wbw-resource://{package_path}");
            }
        }
        _ => {}
    }
}

fn restore_portable_resources(value: &mut Value, extracted: &std::collections::HashMap<String, PathBuf>) {
    match value {
        Value::Object(object) => for child in object.values_mut() { restore_portable_resources(child, extracted); },
        Value::Array(values) => for child in values { restore_portable_resources(child, extracted); },
        Value::String(text) => {
            if let Some(package_path) = text.strip_prefix("wbw-resource://")
                && let Some(path) = extracted.get(package_path)
            {
                *text = path.to_string_lossy().into_owned();
            }
        }
        _ => {}
    }
}

fn safe_component(value: &str) -> String {
''')

# Import resources/ alongside manuscript/media.
repl('src/world.rs', '''            } else if entry_name.starts_with("media/") {
                let file_name = Path::new(&entry_name).file_name().and_then(|v| v.to_str()).map(safe_component).unwrap_or_else(generate_id);
                Some(documents.join(".zen-media").join(safe_component(&name)).join(file_name))
            } else {
''', '''            } else if entry_name.starts_with("media/") || entry_name.starts_with("resources/") {
                let file_name = Path::new(&entry_name).file_name().and_then(|v| v.to_str()).map(safe_component).unwrap_or_else(generate_id);
                let bucket = if entry_name.starts_with("resources/") { ".zen-resources" } else { ".zen-media" };
                Some(documents.join(bucket).join(safe_component(&name)).join(file_name))
            } else {
''')
repl('src/world.rs', '''        if let Some(sections) = root.get_mut("sections").and_then(Value::as_object_mut) {
''', '''        restore_portable_resources(&mut root, &extracted);
        if let Some(sections) = root.get_mut("sections").and_then(Value::as_object_mut) {
''')

# Export recursive resources after explicit manuscript/media collection.
repl('src/world.rs', '''        zip.start_file(PACKAGE_MANIFEST, options)?;
''', '''        let mut resource_counter = 0usize;
        portableize_resources(&mut packaged, &mut files, &mut resource_counter);

        zip.start_file(PACKAGE_MANIFEST, options)?;
''')

# ---------------- wbw_tools.rs ----------------
# Review uses user-configurable filler list from Settings.
repl('src/wbw_tools.rs', '''            inspect_text(&row.label, &text, &mut findings);
''', '''            let configured = workspace.setting_list("review_fillers");
            inspect_text(&row.label, &text, &configured, &mut findings);
''')
repl('src/wbw_tools.rs', '''fn inspect_text(title: &str, text: &str, findings: &mut Vec<(String, String, String)>) {
''', '''fn inspect_text(title: &str, text: &str, configured_fillers: &[String], findings: &mut Vec<(String, String, String)>) {
''')
repl('src/wbw_tools.rs', '''    let fillers = ["realmente", "básicamente", "literalmente", "simplemente", "de alguna manera", "en realidad", "de hecho"];
    let lower = text.to_lowercase();
    for filler in fillers {
        let count = lower.matches(filler).count();
''', '''    let defaults = ["realmente", "básicamente", "literalmente", "simplemente", "de alguna manera", "en realidad", "de hecho"];
    let fillers = if configured_fillers.is_empty() { defaults.iter().map(|value| (*value).to_owned()).collect::<Vec<_>>() } else { configured_fillers.to_vec() };
    let lower = text.to_lowercase();
    for filler in fillers {
        let count = lower.matches(&filler).count();
''')
# Ensure formatting uses owned filler okay; format macro borrows.

# Focused relationship graph + hidden cards.
repl('src/wbw_tools.rs', '''pub fn relationship_board(workspace: &ProjectWorkspace, width: usize, zoom: i32, offset_x: i32, offset_y: i32) -> Vec<String> {
''', '''pub fn relationship_board(workspace: &ProjectWorkspace, width: usize, zoom: i32, offset_x: i32, offset_y: i32, focus_index: Option<usize>) -> Vec<String> {
''')
repl('src/wbw_tools.rs', '''    for (index, row) in characters.iter().enumerate() {
        let fields = workspace.record_fields(WorldSection::Characters, index);
        let id = field(&fields, "id").unwrap_or_else(|| format!("character-{index}"));
''', '''    let focus_id = focus_index.and_then(|index| workspace.character_id(index));
    let mut allowed = std::collections::HashSet::<String>::new();
    if let Some(focus) = focus_id.as_ref() {
        allowed.insert(focus.clone());
        for index in 0..workspace.rows(WorldSection::Relationships).len() {
            let fields = workspace.record_fields(WorldSection::Relationships, index);
            let from = field(&fields, "from").unwrap_or_default();
            let to = field(&fields, "to").unwrap_or_default();
            if &from == focus { allowed.insert(to); }
            if &to == focus { allowed.insert(from); }
        }
    }

    for (index, row) in characters.iter().enumerate() {
        let fields = workspace.record_fields(WorldSection::Characters, index);
        let id = field(&fields, "id").unwrap_or_else(|| format!("character-{index}"));
        let visible = field(&fields, "visible").map(|value| value != "false").unwrap_or(true);
        if !visible || (!allowed.is_empty() && !allowed.contains(&id)) { continue; }
''')
# Skip relationships outside focus set.
repl('src/wbw_tools.rs', '''        let visible = field(&fields, "visible").map(|v| v != "false").unwrap_or(true);
        if !visible { continue; }
''', '''        let visible = field(&fields, "visible").map(|v| v != "false").unwrap_or(true);
        if !visible || (!allowed.is_empty() && (!allowed.contains(&from) || !allowed.contains(&to))) { continue; }
''')

# Layout preview.
repl('src/wbw_tools.rs', '''pub fn timeline_preview(workspace: &ProjectWorkspace, width: usize) -> Vec<String> {
''', '''pub fn layout_preview(workspace: &ProjectWorkspace, width: usize) -> Vec<String> {
    let fields = workspace.record_fields(WorldSection::Layout, 0);
    let preset = field(&fields, "preset").unwrap_or_else(|| "Editorial".into());
    let page_w = field(&fields, "paper_width_mm").unwrap_or_else(|| "152".into());
    let page_h = field(&fields, "paper_height_mm").unwrap_or_else(|| "229".into());
    let font = field(&fields, "font_family").unwrap_or_else(|| "EB Garamond".into());
    let size = field(&fields, "font_size_pt").unwrap_or_else(|| "11.5".into());
    let separator = field(&fields, "scene_separator").unwrap_or_else(|| "⁂".into());
    let width = width.clamp(48, 100);
    let paper = (width.saturating_sub(16)).clamp(34, 70);
    let inner = paper.saturating_sub(4);
    let mut lines = Vec::new();
    lines.push(format!("{preset} · {page_w}×{page_h} mm · {font} {size} pt"));
    lines.push(format!("┌{}┐", "─".repeat(paper)));
    lines.push(format!("│  {:^inner$}  │", "CAPÍTULO XII", inner=inner));
    lines.push(format!("│{}│", " ".repeat(paper)));
    lines.push(format!("│  {:<inner$}  │", "La lluvia golpeaba los postigos mientras", inner=inner));
    lines.push(format!("│  {:<inner$}  │", "la ciudad apagaba una a una sus luces.", inner=inner));
    lines.push(format!("│{}│", " ".repeat(paper)));
    lines.push(format!("│  {:^inner$}  │", separator, inner=inner));
    lines.push(format!("│{}│", " ".repeat(paper)));
    lines.push(format!("│  {:<inner$}  │", "Nadie oyó abrirse la puerta.", inner=inner));
    lines.push(format!("└{}┘", "─".repeat(paper)));
    lines
}

pub fn timeline_preview(workspace: &ProjectWorkspace, width: usize) -> Vec<String> {
''')

# ---------------- app.rs ----------------
# State for focus, compact scene board, marker type.
repl('src/app.rs', '''    workspace_delete_confirm: bool,
    message: Option<String>,
''', '''    workspace_delete_confirm: bool,
    workspace_visual_focus: bool,
    workspace_scene_compact: bool,
    workspace_marker_type: usize,
    message: Option<String>,
''')
repl('src/app.rs', '''            workspace_delete_confirm: false,
            message: None,
''', '''            workspace_delete_confirm: false,
            workspace_visual_focus: false,
            workspace_scene_compact: false,
            workspace_marker_type: 5,
            message: None,
''')

# Visual controls focus/visibility/marker cycling.
repl('src/app.rs', '''                    KeyCode::Char('m') | KeyCode::Char('M') if section == WorldSection::Maps => {
                        if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                            let count = workspace.record_fields(WorldSection::Maps, self.launcher.workspace_row).len();
                            let _ = workspace.add_map_marker(self.launcher.workspace_row, &format!("Lugar {}", count + 1), "Lugar", self.launcher.workspace_visual_x, self.launcher.workspace_visual_y);
                        }
                    }
''', '''                    KeyCode::Char('h') | KeyCode::Char('H') if section == WorldSection::Characters => {
                        if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                            match workspace.toggle_character_visibility(self.launcher.workspace_row) {
                                Ok(visible) => self.launcher.message = Some(if visible { "Ficha visible.".into() } else { "Ficha oculta.".into() }),
                                Err(error) => self.launcher.message = Some(format!("No se pudo cambiar visibilidad: {error}")),
                            }
                        }
                    }
                    KeyCode::Char('f') | KeyCode::Char('F') if section == WorldSection::Characters => {
                        self.launcher.workspace_visual_focus = !self.launcher.workspace_visual_focus;
                    }
                    KeyCode::Char('[') if section == WorldSection::Maps => self.launcher.workspace_marker_type = self.launcher.workspace_marker_type.saturating_sub(1),
                    KeyCode::Char(']') if section == WorldSection::Maps => self.launcher.workspace_marker_type = (self.launcher.workspace_marker_type + 1).min(5),
                    KeyCode::Char('m') | KeyCode::Char('M') if section == WorldSection::Maps => {
                        if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                            let types = ["Capital", "Ciudad", "Ruina", "Puerto", "Fortaleza", "Lugar"];
                            let marker_type = types[self.launcher.workspace_marker_type.min(types.len() - 1)];
                            let count = workspace.record_fields(WorldSection::Maps, self.launcher.workspace_row).len();
                            let _ = workspace.add_map_marker(self.launcher.workspace_row, &format!("{marker_type} {}", count + 1), marker_type, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y);
                            self.launcher.message = Some(format!("Marcador {marker_type} añadido."));
                        }
                    }
''')

# Scene compact mode and import/reference import.
repl('src/app.rs', '''                KeyCode::Char('t') | KeyCode::Char('T') if section == WorldSection::Review => {
''', '''                KeyCode::Char('c') | KeyCode::Char('C') if self.launcher.workspace_detail && section == WorldSection::Scenes => {
                    self.launcher.workspace_scene_compact = !self.launcher.workspace_scene_compact;
                }
                KeyCode::Char('i') | KeyCode::Char('I') if self.launcher.workspace_detail && section == WorldSection::Manuscript => {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Manuscrito", &["txt", "text", "md", "markdown", "docx"]).pick_file() {
                        match world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                            .and_then(|mut workspace| workspace.import_chapter(&path))
                        {
                            Ok(target) => self.launcher.message = Some(format!("Capítulo importado: {}", target.display())),
                            Err(error) => self.launcher.message = Some(format!("No se pudo importar capítulo: {error}")),
                        }
                    }
                }
                KeyCode::Char('i') | KeyCode::Char('I') if self.launcher.workspace_detail && section == WorldSection::References => {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        let title = path.file_stem().and_then(|value| value.to_str()).unwrap_or("Referencia").to_owned();
                        if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                            let index = workspace.rows(WorldSection::References).len();
                            if let Err(error) = workspace.add_record(WorldSection::References, &title)
                                .and_then(|_| workspace.set_field(WorldSection::References, index, "target", &path.to_string_lossy()).map(|_| ()))
                                .and_then(|_| workspace.set_field(WorldSection::References, index, "source_name", path.file_name().and_then(|value| value.to_str()).unwrap_or("Referencia")).map(|_| ()))
                            {
                                self.launcher.message = Some(format!("No se pudo importar referencia: {error}"));
                            } else {
                                self.launcher.message = Some(format!("Referencia importada: {title}"));
                            }
                        }
                    }
                }
                KeyCode::Char('t') | KeyCode::Char('T') if section == WorldSection::Review => {
''')

# Layout visual mode.
repl('src/app.rs', '''                KeyCode::Char('v') | KeyCode::Char('V') if self.launcher.workspace_detail && matches!(section, WorldSection::Characters | WorldSection::Relationships | WorldSection::Maps | WorldSection::Timeline) => {
''', '''                KeyCode::Char('v') | KeyCode::Char('V') if self.launcher.workspace_detail && matches!(section, WorldSection::Characters | WorldSection::Relationships | WorldSection::Maps | WorldSection::Timeline | WorldSection::Layout) => {
''')

# Render focused board/layout preview.
repl('src/app.rs', '''                        let lines = if section == WorldSection::Maps {
                            wbw_tools::map_preview(&workspace, self.launcher.workspace_row, inner_width.saturating_sub(2), self.launcher.workspace_visual_zoom, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y)
                        } else if section == WorldSection::Timeline {
                            wbw_tools::timeline_preview(&workspace, inner_width.saturating_sub(2))
                        } else {
                            wbw_tools::relationship_board(&workspace, inner_width.saturating_sub(2), self.launcher.workspace_visual_zoom, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y)
                        };
''', '''                        let lines = if section == WorldSection::Maps {
                            wbw_tools::map_preview(&workspace, self.launcher.workspace_row, inner_width.saturating_sub(2), self.launcher.workspace_visual_zoom, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y)
                        } else if section == WorldSection::Timeline {
                            wbw_tools::timeline_preview(&workspace, inner_width.saturating_sub(2))
                        } else if section == WorldSection::Layout {
                            wbw_tools::layout_preview(&workspace, inner_width.saturating_sub(2))
                        } else {
                            wbw_tools::relationship_board(&workspace, inner_width.saturating_sub(2), self.launcher.workspace_visual_zoom, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y, if self.launcher.workspace_visual_focus { Some(self.launcher.workspace_row) } else { None })
                        };
''')
repl('src/app.rs', '''                        let visual_help = if section == WorldSection::Characters { "Flechas pan · Shift+flechas mover ficha · +/- zoom · V/Esc volver" }
                            else if section == WorldSection::Maps { "Flechas pan · +/- zoom · M marcador · V/Esc volver" }
                            else { "Flechas pan · +/- zoom · V/Esc volver" };
''', '''                        let visual_help = if section == WorldSection::Characters { "Flechas pan · Shift+flechas mover · H ocultar · F red enfocada · +/- zoom · V/Esc" }
                            else if section == WorldSection::Maps { "Flechas pan · +/- zoom · [/] tipo marcador · M añadir · V/Esc" }
                            else if section == WorldSection::Layout { "V/Esc volver" }
                            else { "Flechas pan · +/- zoom · V/Esc volver" };
''')

# Scene board uses viewport and compact mode.
old = '''                        } else if section == WorldSection::Scenes {
                            for (index, row) in rows.iter().enumerate().take((rows.len()).min(8)) {
                                let marker = if index == self.launcher.workspace_row { "▶" } else { " " };
                                let fields = workspace.record_fields(section, index);
                                let get = |key: &str| fields.iter().find(|field| field.label == key).map(|field| field.detail.as_str()).unwrap_or("");
                                let chapter = get("chapter");
                                let pov = get("pov");
                                let location = get("location");
                                let status = get("status");
                                let head = format!("{marker} ┌ {}  [cap. {} · {}]", row.label, if chapter.is_empty() { "—" } else { chapter }, status);
                                let meta = format!("  └ POV: {} · Lugar: {}", if pov.is_empty() { "—" } else { pov }, if location.is_empty() { "—" } else { location });
                                push_line(&mut out, &framed_left(&truncate(&head, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                                push_line(&mut out, &framed_left(&truncate(&meta, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                            }
'''
new = '''                        } else if section == WorldSection::Scenes {
                            let visible = if self.launcher.workspace_scene_compact { 12 } else { 7 };
                            let start = self.launcher.workspace_row.saturating_sub(visible.saturating_sub(1));
                            for (index, row) in rows.iter().enumerate().skip(start).take(visible) {
                                let marker = if index == self.launcher.workspace_row { "▶" } else { " " };
                                let fields = workspace.record_fields(section, index);
                                let get = |key: &str| fields.iter().find(|field| field.label == key).map(|field| field.detail.as_str()).unwrap_or("");
                                let chapter = get("chapter");
                                let pov = get("pov");
                                let location = get("location");
                                let status = get("status");
                                if self.launcher.workspace_scene_compact {
                                    let line = format!("{marker} {} · cap. {} · {} · POV {} · {}", row.label, if chapter.is_empty() { "—" } else { chapter }, status, if pov.is_empty() { "—" } else { pov }, if location.is_empty() { "—" } else { location });
                                    push_line(&mut out, &framed_left(&truncate(&line, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                                } else {
                                    let head = format!("{marker} ┌ {}  [cap. {} · {}]", row.label, if chapter.is_empty() { "—" } else { chapter }, status);
                                    let meta = format!("  └ POV: {} · Lugar: {}", if pov.is_empty() { "—" } else { pov }, if location.is_empty() { "—" } else { location });
                                    push_line(&mut out, &framed_left(&truncate(&head, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                                    push_line(&mut out, &framed_left(&truncate(&meta, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                                }
                            }
'''
repl('src/app.rs', old, new)
# Standard lists scroll with selection instead of disappearing after first 14.
repl('src/app.rs', '''                            for (index, row) in rows.iter().enumerate().take((rows.len()).min(14)) {
                                let marker = if index == self.launcher.workspace_row { "▶" } else { " " };
''', '''                            let start = self.launcher.workspace_row.saturating_sub(12);
                            for (index, row) in rows.iter().enumerate().skip(start).take(14) {
                                let marker = if index == self.launcher.workspace_row { "▶" } else { " " };
''')

# UI hints: Scenes compact/import refs/layout preview/manuscript import.
repl('src/app.rs', '''                            let visual = if matches!(section, WorldSection::Characters | WorldSection::Relationships | WorldSection::Maps | WorldSection::Timeline) { " · V vista" } else { "" };
                            let review = if section == WorldSection::Review { " · T revisar" } else { "" };
                            let open = if matches!(section, WorldSection::Media | WorldSection::References) { " · O abrir" } else { "" };
                            push_line(&mut out, &framed_left(&format!("N nuevo · Enter ficha · R renombrar · D eliminar · Ctrl+↑↓ mover{extra}{visual}{review}{open} · P PDF · Esc volver"), inner_width, "38;5;244"));
''', '''                            let visual = if matches!(section, WorldSection::Characters | WorldSection::Relationships | WorldSection::Maps | WorldSection::Timeline) { " · V vista" } else { "" };
                            let review = if section == WorldSection::Review { " · T revisar" } else { "" };
                            let open = if matches!(section, WorldSection::Media | WorldSection::References) { " · O abrir" } else { "" };
                            let compact = if section == WorldSection::Scenes { " · C compacto" } else { "" };
                            let import = if section == WorldSection::References { " · I importar" } else { "" };
                            push_line(&mut out, &framed_left(&format!("N nuevo · Enter ficha · R renombrar · D eliminar · Ctrl+↑↓ mover{extra}{visual}{review}{open}{compact}{import} · P PDF · Esc volver"), inner_width, "38;5;244"));
''')
repl('src/app.rs', '''                            push_line(&mut out, &framed_left("N capítulo · Enter abrir · D eliminar · Ctrl+↑↓ reordenar · P PDF · Esc volver", inner_width, "38;5;244"));
''', '''                            push_line(&mut out, &framed_left("N capítulo · I importar TXT/MD/DOCX · Enter abrir · D eliminar · Ctrl+↑↓ reordenar · P PDF · Esc", inner_width, "38;5;244"));
''')
repl('src/app.rs', '''                            push_line(&mut out, &framed_left("1 Editorial · 2 Amplia · 3 Bolsillo · 4 Fantasía · 5 Crónica · 6 Personalizada · Enter editar", inner_width, "38;5;244"));
''', '''                            push_line(&mut out, &framed_left("1 Editorial · 2 Amplia · 3 Bolsillo · 4 Fantasía · 5 Crónica · 6 Personalizada · V previsualizar · Enter editar", inner_width, "38;5;244"));
''')

# Changelog.
p = Path('CHANGELOG.md')
text = p.read_text(encoding='utf-8')
entry = '''## WBW inventory closure — 2026-10-17\n\n- Importación de capítulos TXT/Markdown/DOCX desde Manuscrito conservando el formato estructurado convertido por Zen.\n- Tablero de escenas con viewport real y modo compacto; palabras/fecha de actualización sincronizadas.\n- Tablero de personajes permite ocultar fichas y enfocar la red en un personaje y sus relaciones.\n- Tipos de marcador Capital/Ciudad/Ruina/Puerto/Fortaleza/Lugar y previsualización de maquetación.\n- Lista de muletillas configurable desde Settings utilizada por la revisión automática.\n- Portabilidad `.wbw` ampliada a portada, banner, icono, imágenes, fondos y adjuntos locales bajo `resources/`.\n- Importación directa de textos de referencia y listas largas con viewport siguiendo la selección.\n\n'''
if '## WBW inventory closure — 2026-10-17' not in text:
    marker = text.find('## ')
    text = text[:marker] + entry + text[marker:] if marker >= 0 else entry + text
    p.write_text(text, encoding='utf-8')
