from pathlib import Path


def repl(path, old, new):
    p = Path(path)
    text = p.read_text(encoding='utf-8')
    if old not in text:
        raise SystemExit(f'missing pattern in {path}: {old[:180]!r}')
    p.write_text(text.replace(old, new, 1), encoding='utf-8')

# --- world.rs: native manuscript operations, markers, complete counters and config ---
repl('src/world.rs', '''        summary.words = self
            .array(WorldSection::Manuscript)
            .iter()
            .filter_map(|row| row.get("words").and_then(Value::as_u64))
            .map(|value| value as usize)
            .sum();
''', '''        summary.words = self
            .array(WorldSection::Manuscript)
            .iter()
            .chain(self.array(WorldSection::Scenes).iter())
            .filter_map(|row| row.get("words").and_then(Value::as_u64))
            .map(|value| value as usize)
            .sum();
''')

repl('src/world.rs', '''    pub fn add_record(&mut self, section: WorldSection, title: &str) -> Result<()> {
''', '''    pub fn create_chapter(&mut self, title: &str) -> Result<PathBuf> {
        let index = self.array(WorldSection::Manuscript).len();
        let chapter = index as u32 + 1;
        let order = (index as i64 + 1) * 10;
        let path = unique_scene_path(&self.documents, &self.name, title);
        let document = document::HsstDocument {
            metadata: document::DocumentMetadata {
                format: document::FORMAT_VERSION,
                id: generate_id(),
                title: title.to_owned(),
                project: self.name.clone(),
                kind: "chapter".into(),
                chapter: Some(chapter),
                order,
                language: "es-CL".into(),
                status: "draft".into(),
                page: crate::page::PageProfile::default(),
            },
            body: String::new(),
            formatting: json!({"version": 1, "unit": "utf8-byte", "runs": [], "paragraphs": []}),
        };
        document::write(&path, &document)?;
        self.sync_manuscript(&self.documents.clone())?;
        self.touch();
        self.save()?;
        Ok(path)
    }

    pub fn delete_manuscript(&mut self, index: usize) -> Result<bool> {
        let Some(path) = self.array(WorldSection::Manuscript).get(index).and_then(|row| row.get("path")).and_then(Value::as_str).map(PathBuf::from) else {
            return Ok(false);
        };
        if path.exists() { fs::remove_file(&path)?; }
        let sidecar = path.with_file_name(format!("{}.zen.json", path.file_name().and_then(|v| v.to_str()).unwrap_or("chapter.txt")));
        let _ = fs::remove_file(sidecar);
        self.sync_manuscript(&self.documents.clone())?;
        self.renumber_chapters()?;
        self.touch();
        self.save()?;
        Ok(true)
    }

    pub fn move_manuscript(&mut self, index: usize, delta: isize) -> Result<usize> {
        let rows = self.array(WorldSection::Manuscript);
        if rows.is_empty() || index >= rows.len() { return Ok(index); }
        let target = (index as isize + delta).clamp(0, rows.len() as isize - 1) as usize;
        if target == index { return Ok(index); }
        let mut paths = rows.iter().filter_map(|row| row.get("path").and_then(Value::as_str).map(PathBuf::from)).collect::<Vec<_>>();
        let moved = paths.remove(index);
        paths.insert(target, moved);
        for (position, path) in paths.iter().enumerate() {
            if let Ok(mut doc) = document::read(path) {
                doc.metadata.order = (position as i64 + 1) * 10;
                doc.metadata.chapter = Some(position as u32 + 1);
                document::write(path, &doc)?;
            }
        }
        self.sync_manuscript(&self.documents.clone())?;
        self.touch();
        self.save()?;
        Ok(target)
    }

    fn renumber_chapters(&mut self) -> Result<()> {
        let paths = self.array(WorldSection::Manuscript).iter().filter_map(|row| row.get("path").and_then(Value::as_str).map(PathBuf::from)).collect::<Vec<_>>();
        for (position, path) in paths.iter().enumerate() {
            if let Ok(mut doc) = document::read(path) {
                doc.metadata.order = (position as i64 + 1) * 10;
                doc.metadata.chapter = Some(position as u32 + 1);
                document::write(path, &doc)?;
            }
        }
        self.sync_manuscript(&self.documents.clone())
    }

    pub fn add_map_marker(&mut self, map_index: usize, label: &str, marker_type: &str, x: i32, y: i32) -> Result<bool> {
        let Some(map) = self.array_mut(WorldSection::Maps).get_mut(map_index).and_then(Value::as_object_mut) else { return Ok(false); };
        let markers = map.entry("markers").or_insert_with(|| json!([])).as_array_mut().ok_or_else(|| anyhow::anyhow!("markers inválido"))?;
        markers.push(json!({"id": generate_id(), "name": label, "type": marker_type, "x": x, "y": y}));
        self.touch();
        self.save()?;
        Ok(true)
    }

    pub fn move_character(&mut self, index: usize, dx: i32, dy: i32) -> Result<bool> {
        let Some(character) = self.array_mut(WorldSection::Characters).get_mut(index).and_then(Value::as_object_mut) else { return Ok(false); };
        let x = character.get("x").and_then(Value::as_i64).unwrap_or(0) as i32 + dx;
        let y = character.get("y").and_then(Value::as_i64).unwrap_or(0) as i32 + dy;
        character.insert("x".into(), json!(x));
        character.insert("y".into(), json!(y));
        self.touch();
        self.save()?;
        Ok(true)
    }

    pub fn add_record(&mut self, section: WorldSection, title: &str) -> Result<()> {
''')

repl('src/world.rs', '''                    "status": "draft", "summary": "", "words": 0, "order": order
''', '''                    "status": "draft", "summary": "", "words": 0, "updated": unix_seconds(), "order": order
''')

repl('src/world.rs', '''        "theme": "dark", "autosave": true, "spellcheck": true, "language": "es-CL",
        "focus_fullscreen": true, "backup_local": true, "backup_drive": false,
        "editor_font": "JetBrains Mono", "editor_font_size": 14,
        "map_style": "Parchment", "map_grid": true
''', '''        "theme": "dark", "theme_background": "#282828", "theme_foreground": "#dfe8ef", "theme_accent": "#e8cc83",
        "autosave": true, "spellcheck": true, "language": "es-CL",
        "focus_fullscreen": true, "backup_local": true, "backup_drive": false,
        "editor_font": "JetBrains Mono", "editor_font_size": 14,
        "map_style": "Parchment", "map_grid": true,
        "review_fillers": ["realmente", "básicamente", "literalmente", "simplemente", "de alguna manera", "en realidad", "de hecho"]
''')

# Refresh scene word counts/update timestamps when workspace is loaded.
repl('src/world.rs', '''        for (index, path) in replacements {
            if let Some(object) = self.array_mut(WorldSection::Scenes).get_mut(index).and_then(Value::as_object_mut) {
                object.insert("path".into(), json!(path.to_string_lossy()));
            }
        }
        Ok(())
    }
''', '''        for (index, path) in replacements {
            if let Some(object) = self.array_mut(WorldSection::Scenes).get_mut(index).and_then(Value::as_object_mut) {
                object.insert("path".into(), json!(path.to_string_lossy()));
            }
        }
        let snapshots = self.array(WorldSection::Scenes).iter().enumerate().filter_map(|(index, row)| {
            let path = row.get("path").and_then(Value::as_str).map(PathBuf::from)?;
            let words = document::read(&path).map(|doc| doc.body.split_whitespace().count()).unwrap_or(0);
            Some((index, words))
        }).collect::<Vec<_>>();
        for (index, words) in snapshots {
            if let Some(object) = self.array_mut(WorldSection::Scenes).get_mut(index).and_then(Value::as_object_mut) {
                object.insert("words".into(), json!(words));
            }
        }
        Ok(())
    }
''')

# --- wbw_tools.rs: configurable review, timeline view, stronger relation rendering, presets ---
repl('src/wbw_tools.rs', '''    let fillers = [
        "realmente",
        "básicamente",
        "literalmente",
        "simplemente",
        "de alguna manera",
        "en realidad",
        "de hecho",
    ];
    let lower = text.to_lowercase();
    for filler in fillers {
''', '''    let fillers = ["realmente", "básicamente", "literalmente", "simplemente", "de alguna manera", "en realidad", "de hecho"];
    let lower = text.to_lowercase();
    for filler in fillers {
''')

# relationship line character by type/strength
repl('src/wbw_tools.rs', '''        let visible = field(&fields, "visible").map(|v| v != "false").unwrap_or(true);
        if !visible { continue; }
        let (Some((x1, y1, _)), Some((x2, y2, _))) = (positions.get(&from), positions.get(&to)) else { continue; };
        draw_line(&mut canvas, *x1, *y1, *x2, *y2, '·');
''', '''        let visible = field(&fields, "visible").map(|v| v != "false").unwrap_or(true);
        if !visible { continue; }
        let relation_type = field(&fields, "type").unwrap_or_default().to_lowercase();
        let strength = field(&fields, "strength").and_then(|v| v.parse::<i32>().ok()).unwrap_or(0).abs();
        let glyph = if relation_type.contains("enem") || relation_type.contains("conflict") { '!' }
            else if relation_type.contains("famil") || relation_type.contains("amor") { '=' }
            else if strength >= 7 { '━' } else if strength >= 4 { '-' } else { '·' };
        let (Some((x1, y1, _)), Some((x2, y2, _))) = (positions.get(&from), positions.get(&to)) else { continue; };
        draw_line(&mut canvas, *x1, *y1, *x2, *y2, glyph);
''')

# insert timeline/presets before open_external
repl('src/wbw_tools.rs', '''pub fn open_external(target: &str) -> Result<()> {
''', '''pub fn timeline_preview(workspace: &ProjectWorkspace, width: usize) -> Vec<String> {
    let width = width.clamp(48, 120);
    let mut entries = (0..workspace.rows(WorldSection::Timeline).len()).map(|index| {
        let fields = workspace.record_fields(WorldSection::Timeline, index);
        let title = field(&fields, "title").unwrap_or_else(|| "Evento".into());
        let position = field(&fields, "position").unwrap_or_default();
        let intensity = field(&fields, "intensity").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0).min(10);
        let order = field(&fields, "order").and_then(|v| v.parse::<i64>().ok()).unwrap_or(index as i64);
        (order, position, title, intensity)
    }).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.0);
    entries.into_iter().map(|(_, position, title, intensity)| {
        let bar = "█".repeat(intensity.max(1));
        truncate_chars(&format!("{:>12} ── {:<28} {}", if position.is_empty() { "—" } else { &position }, title, bar), width)
    }).collect()
}

pub fn apply_layout_preset(workspace: &mut ProjectWorkspace, preset: usize) -> Result<()> {
    let values = match preset {
        1 => ("Editorial", "152", "229", "11.5", "1.2", "1.2", "⁂"),
        2 => ("Novela amplia", "170", "240", "12", "1.25", "1.25", "⁂"),
        3 => ("Bolsillo", "110", "178", "10", "1.15", "1.0", "* * *"),
        4 => ("Fantasía clásica", "156", "234", "11.5", "1.22", "1.3", "❦"),
        5 => ("Crónica ilustrada", "190", "250", "11", "1.3", "1.0", "◆"),
        _ => ("Personalizada", "152", "229", "11.5", "1.2", "1.2", "⁂"),
    };
    for (key, value) in [
        ("preset", values.0), ("paper_width_mm", values.1), ("paper_height_mm", values.2),
        ("font_size_pt", values.3), ("line_height", values.4), ("indent_em", values.5), ("scene_separator", values.6)
    ] {
        let _ = workspace.set_field(WorldSection::Layout, 0, key, value)?;
    }
    Ok(())
}

pub fn apply_theme_preset(workspace: &mut ProjectWorkspace, preset: usize) -> Result<()> {
    let values = match preset {
        1 => ("dark", "#282828", "#dfe8ef", "#e8cc83"),
        2 => ("gruvbox", "#282828", "#ebdbb2", "#fabd2f"),
        3 => ("paper", "#f2eadf", "#2f2b27", "#7a4f2a"),
        _ => ("custom", "#282828", "#dfe8ef", "#8db9bb"),
    };
    for (key, value) in [("theme", values.0), ("theme_background", values.1), ("theme_foreground", values.2), ("theme_accent", values.3)] {
        let _ = workspace.set_field(WorldSection::Settings, 0, key, value)?;
    }
    Ok(())
}

pub fn open_external(target: &str) -> Result<()> {
''')

# remove unused helper if it remains
p = Path('src/wbw_tools.rs')
text = p.read_text(encoding='utf-8')
start = text.find('\npub fn first_project_source(')
if start >= 0:
    text = text[:start].rstrip() + '\n'
p.write_text(text, encoding='utf-8')

# --- app.rs parity controls ---
# New manuscript entries use native chapter documents.
repl('src/app.rs', '''                            match world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                                .and_then(|mut workspace| workspace.add_record(section, &title))
                            {
                                Ok(()) => self.launcher.message = Some(format!("{} añadido a {}", title, section.label())),
''', '''                            let result = world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                                .and_then(|mut workspace| {
                                    if section == WorldSection::Manuscript { workspace.create_chapter(&title).map(|_| ()) }
                                    else { workspace.add_record(section, &title) }
                                });
                            match result {
                                Ok(()) => self.launcher.message = Some(format!("{} añadido a {}", title, section.label())),
''')
repl('src/app.rs', '''                KeyCode::Char('n') | KeyCode::Char('N') if self.launcher.workspace_detail && section.is_record_list() => {
''', '''                KeyCode::Char('n') | KeyCode::Char('N') if self.launcher.workspace_detail && (section.is_record_list() || section == WorldSection::Manuscript) => {
''')

# Move/delete manuscript with same controls.
repl('src/app.rs', '''                KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) && self.launcher.workspace_detail && section.is_record_list() => {
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        if let Ok(index) = workspace.move_record(section, self.launcher.workspace_row, -1) { self.launcher.workspace_row = index; }
                    }
                }
                KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) && self.launcher.workspace_detail && section.is_record_list() => {
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        if let Ok(index) = workspace.move_record(section, self.launcher.workspace_row, 1) { self.launcher.workspace_row = index; }
                    }
                }
''', '''                KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) && self.launcher.workspace_detail && (section.is_record_list() || section == WorldSection::Manuscript) => {
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        let result = if section == WorldSection::Manuscript { workspace.move_manuscript(self.launcher.workspace_row, -1) }
                            else { workspace.move_record(section, self.launcher.workspace_row, -1) };
                        if let Ok(index) = result { self.launcher.workspace_row = index; }
                    }
                }
                KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) && self.launcher.workspace_detail && (section.is_record_list() || section == WorldSection::Manuscript) => {
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        let result = if section == WorldSection::Manuscript { workspace.move_manuscript(self.launcher.workspace_row, 1) }
                            else { workspace.move_record(section, self.launcher.workspace_row, 1) };
                        if let Ok(index) = result { self.launcher.workspace_row = index; }
                    }
                }
''')
repl('src/app.rs', '''                KeyCode::Char('d') | KeyCode::Char('D') if self.launcher.workspace_detail && section.is_record_list() => {
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        match workspace.delete_record(section, self.launcher.workspace_row) {
''', '''                KeyCode::Char('d') | KeyCode::Char('D') if self.launcher.workspace_detail && (section.is_record_list() || section == WorldSection::Manuscript) => {
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        let result = if section == WorldSection::Manuscript { workspace.delete_manuscript(self.launcher.workspace_row) }
                            else { workspace.delete_record(section, self.launcher.workspace_row) };
                        match result {
''')

# Visual mode supports timeline, moving character cards, adding map markers.
repl('src/app.rs', '''                    KeyCode::Left => self.launcher.workspace_visual_x -= 2,
                    KeyCode::Right => self.launcher.workspace_visual_x += 2,
                    KeyCode::Up => self.launcher.workspace_visual_y -= 1,
                    KeyCode::Down => self.launcher.workspace_visual_y += 1,
                    _ => {}
''', '''                    KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) && section == WorldSection::Characters => {
                        if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) { let _ = workspace.move_character(self.launcher.workspace_row, -2, 0); }
                    }
                    KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) && section == WorldSection::Characters => {
                        if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) { let _ = workspace.move_character(self.launcher.workspace_row, 2, 0); }
                    }
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) && section == WorldSection::Characters => {
                        if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) { let _ = workspace.move_character(self.launcher.workspace_row, 0, -1); }
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) && section == WorldSection::Characters => {
                        if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) { let _ = workspace.move_character(self.launcher.workspace_row, 0, 1); }
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') if section == WorldSection::Maps => {
                        if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                            let count = workspace.record_fields(WorldSection::Maps, self.launcher.workspace_row).len();
                            let _ = workspace.add_map_marker(self.launcher.workspace_row, &format!("Lugar {}", count + 1), "Lugar", self.launcher.workspace_visual_x, self.launcher.workspace_visual_y);
                        }
                    }
                    KeyCode::Left => self.launcher.workspace_visual_x -= 2,
                    KeyCode::Right => self.launcher.workspace_visual_x += 2,
                    KeyCode::Up => self.launcher.workspace_visual_y -= 1,
                    KeyCode::Down => self.launcher.workspace_visual_y += 1,
                    _ => {}
''')
repl('src/app.rs', '''                KeyCode::Char('v') | KeyCode::Char('V') if self.launcher.workspace_detail && matches!(section, WorldSection::Characters | WorldSection::Relationships | WorldSection::Maps) => {
''', '''                KeyCode::Char('v') | KeyCode::Char('V') if self.launcher.workspace_detail && matches!(section, WorldSection::Characters | WorldSection::Relationships | WorldSection::Maps | WorldSection::Timeline) => {
''')

# Presets and asset/background pickers.
repl('src/app.rs', '''                KeyCode::Char('t') | KeyCode::Char('T') if section == WorldSection::Review => {
''', '''                KeyCode::Char('1'..='6') if self.launcher.workspace_detail && section == WorldSection::Layout => {
                    let preset = match key.code { KeyCode::Char(ch) => ch.to_digit(10).unwrap_or(1) as usize, _ => 1 };
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        match wbw_tools::apply_layout_preset(&mut workspace, preset) {
                            Ok(()) => self.launcher.message = Some(format!("Preset de maquetación {preset} aplicado.")),
                            Err(error) => self.launcher.message = Some(format!("No se pudo aplicar preset: {error}")),
                        }
                    }
                }
                KeyCode::Char('1'..='4') if self.launcher.workspace_detail && section == WorldSection::Settings => {
                    let preset = match key.code { KeyCode::Char(ch) => ch.to_digit(10).unwrap_or(1) as usize, _ => 1 };
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        match wbw_tools::apply_theme_preset(&mut workspace, preset) {
                            Ok(()) => self.launcher.message = Some(format!("Tema {preset} aplicado.")),
                            Err(error) => self.launcher.message = Some(format!("No se pudo aplicar tema: {error}")),
                        }
                    }
                }
                KeyCode::Char('a') | KeyCode::Char('A') if self.launcher.workspace_detail && matches!(section, WorldSection::Project | WorldSection::Maps) => {
                    if let Some(path) = rfd::FileDialog::new().pick_file()
                        && let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                    {
                        if section == WorldSection::Maps {
                            let _ = workspace.set_field(section, self.launcher.workspace_row, "background", &path.to_string_lossy());
                            self.launcher.message = Some("Imagen de fondo del mapa asignada.".into());
                        } else {
                            let rows = workspace.rows(section);
                            let key_name = rows.get(self.launcher.workspace_row).map(|row| row.label.as_str()).filter(|name| matches!(*name, "cover" | "banner" | "icon")).unwrap_or("cover").to_owned();
                            let _ = workspace.set_field(section, 0, &key_name, &path.to_string_lossy());
                            self.launcher.message = Some(format!("{} asignado.", key_name));
                        }
                    }
                }
                KeyCode::Char('t') | KeyCode::Char('T') if section == WorldSection::Review => {
''')

# Visual render supports timeline and richer help.
repl('src/app.rs', '''                        let lines = if section == WorldSection::Maps {
                            wbw_tools::map_preview(&workspace, self.launcher.workspace_row, inner_width.saturating_sub(2), self.launcher.workspace_visual_zoom, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y)
                        } else {
                            wbw_tools::relationship_board(&workspace, inner_width.saturating_sub(2), self.launcher.workspace_visual_zoom, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y)
                        };
''', '''                        let lines = if section == WorldSection::Maps {
                            wbw_tools::map_preview(&workspace, self.launcher.workspace_row, inner_width.saturating_sub(2), self.launcher.workspace_visual_zoom, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y)
                        } else if section == WorldSection::Timeline {
                            wbw_tools::timeline_preview(&workspace, inner_width.saturating_sub(2))
                        } else {
                            wbw_tools::relationship_board(&workspace, inner_width.saturating_sub(2), self.launcher.workspace_visual_zoom, self.launcher.workspace_visual_x, self.launcher.workspace_visual_y)
                        };
''')
repl('src/app.rs', '''                        push_line(&mut out, &framed_left("Flechas mover · +/- zoom · V/Esc volver", inner_width, "38;5;244"));
''', '''                        let visual_help = if section == WorldSection::Characters { "Flechas pan · Shift+flechas mover ficha · +/- zoom · V/Esc volver" }
                            else if section == WorldSection::Maps { "Flechas pan · +/- zoom · M marcador · V/Esc volver" }
                            else { "Flechas pan · +/- zoom · V/Esc volver" };
                        push_line(&mut out, &framed_left(visual_help, inner_width, "38;5;244"));
''')
repl('src/app.rs', '''                            let visual = if matches!(section, WorldSection::Characters | WorldSection::Relationships | WorldSection::Maps) { " · V vista" } else { "" };
''', '''                            let visual = if matches!(section, WorldSection::Characters | WorldSection::Relationships | WorldSection::Maps | WorldSection::Timeline) { " · V vista" } else { "" };
''')

# Manuscript controls and preset hints.
repl('src/app.rs', '''                        } else if matches!(section, WorldSection::Project | WorldSection::Layout | WorldSection::Settings) {
                            push_line(&mut out, &framed_left("Enter editar campo · Esc volver", inner_width, "38;5;244"));
                        } else {
                            push_line(&mut out, &framed_left("Enter abrir · Esc volver", inner_width, "38;5;244"));
                        }
''', '''                        } else if section == WorldSection::Manuscript {
                            push_line(&mut out, &framed_left("N capítulo · Enter abrir · D eliminar · Ctrl+↑↓ reordenar · P PDF · Esc volver", inner_width, "38;5;244"));
                        } else if section == WorldSection::Layout {
                            push_line(&mut out, &framed_left("1 Editorial · 2 Amplia · 3 Bolsillo · 4 Fantasía · 5 Crónica · 6 Personalizada · Enter editar", inner_width, "38;5;244"));
                        } else if section == WorldSection::Settings {
                            push_line(&mut out, &framed_left("1 Dark · 2 Gruvbox · 3 Papel · 4 Custom · Enter editar", inner_width, "38;5;244"));
                        } else if section == WorldSection::Project {
                            push_line(&mut out, &framed_left("Enter editar campo · A adjuntar portada/banner/icono · Esc volver", inner_width, "38;5;244"));
                        } else {
                            push_line(&mut out, &framed_left("Enter abrir · Esc volver", inner_width, "38;5;244"));
                        }
''')

# Changelog
p = Path('CHANGELOG.md')
text = p.read_text(encoding='utf-8')
entry = '''## WBW parity completion — 2026-10-17\n\n- Manuscrito WBW crea, abre, elimina y reordena capítulos TXT nativos, renumerando metadatos.\n- Tablero de personajes permite mover fichas; relaciones diferencian tipo/intensidad; cronología tiene vista visual.\n- Mapas permiten pan/zoom y creación de marcadores, además de imagen de fondo.\n- Presets de maquetación Editorial, Novela amplia, Bolsillo, Fantasía clásica, Crónica ilustrada y Personalizada.\n- Temas Dark, Gruvbox, Papel y Custom; portada/banner/icono pueden asignarse desde selector de archivos.\n- Estadísticas incluyen palabras de capítulos y escenas.\n\n'''
if '## WBW parity completion — 2026-10-17' not in text:
    marker = text.find('## ')
    text = text[:marker] + entry + text[marker:] if marker >= 0 else entry + text
    p.write_text(text, encoding='utf-8')
