from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    s = p.read_text(encoding="utf-8")
    if old not in s:
        raise SystemExit(f"missing pattern in {path}: {old[:160]!r}")
    p.write_text(s.replace(old, new, 1), encoding="utf-8")


replace_once("Cargo.toml", 'version = "0.2.22"', 'version = "0.2.23"')

# ---------- document metadata ----------
p = Path("src/document.rs")
s = p.read_text(encoding="utf-8")
s = s.replace(
    '''    pub status: String,\n    pub page: PageProfile,\n''',
    '''    pub status: String,\n    pub pov: String,\n    pub location: String,\n    pub timeline: String,\n    pub tags: Vec<String>,\n    pub summary: String,\n    pub page: PageProfile,\n''',
    1,
)
s = s.replace(
    '''            status: "draft".into(),\n            page: PageProfile::default(),\n''',
    '''            status: "draft".into(),\n            pov: String::new(),\n            location: String::new(),\n            timeline: String::new(),\n            tags: Vec::new(),\n            summary: String::new(),\n            page: PageProfile::default(),\n''',
    1,
)
s = s.replace(
    '''        "status": metadata.status,\n        "page": {\n''',
    '''        "status": metadata.status,\n        "pov": metadata.pov,\n        "location": metadata.location,\n        "timeline": metadata.timeline,\n        "tags": metadata.tags,\n        "summary": metadata.summary,\n        "page": {\n''',
    1,
)
s = s.replace(
    '''        status: value\n            .get("status")\n            .and_then(Value::as_str)\n            .unwrap_or("draft")\n            .to_owned(),\n        page,\n''',
    '''        status: value\n            .get("status")\n            .and_then(Value::as_str)\n            .unwrap_or("draft")\n            .to_owned(),\n        pov: value.get("pov").and_then(Value::as_str).unwrap_or("").to_owned(),\n        location: value\n            .get("location")\n            .and_then(Value::as_str)\n            .unwrap_or("")\n            .to_owned(),\n        timeline: value\n            .get("timeline")\n            .and_then(Value::as_str)\n            .unwrap_or("")\n            .to_owned(),\n        tags: value\n            .get("tags")\n            .and_then(Value::as_array)\n            .map(|values| {\n                values\n                    .iter()\n                    .filter_map(Value::as_str)\n                    .map(str::to_owned)\n                    .collect()\n            })\n            .unwrap_or_default(),\n        summary: value\n            .get("summary")\n            .and_then(Value::as_str)\n            .unwrap_or("")\n            .to_owned(),\n        page,\n''',
    1,
)
s = s.replace(
    '''        status: table\n            .get("status")\n            .and_then(toml::Value::as_str)\n            .unwrap_or("draft")\n            .to_owned(),\n        page,\n''',
    '''        status: table\n            .get("status")\n            .and_then(toml::Value::as_str)\n            .unwrap_or("draft")\n            .to_owned(),\n        pov: table.get("pov").and_then(toml::Value::as_str).unwrap_or("").to_owned(),\n        location: table\n            .get("location")\n            .and_then(toml::Value::as_str)\n            .unwrap_or("")\n            .to_owned(),\n        timeline: table\n            .get("timeline")\n            .and_then(toml::Value::as_str)\n            .unwrap_or("")\n            .to_owned(),\n        tags: table\n            .get("tags")\n            .and_then(toml::Value::as_array)\n            .map(|values| {\n                values\n                    .iter()\n                    .filter_map(toml::Value::as_str)\n                    .map(str::to_owned)\n                    .collect()\n            })\n            .unwrap_or_default(),\n        summary: table\n            .get("summary")\n            .and_then(toml::Value::as_str)\n            .unwrap_or("")\n            .to_owned(),\n        page,\n''',
    1,
)
# project manifest now carries the literary metadata too
s = s.replace(
    '''                "chapter": metadata.chapter,\n                "order": metadata.order\n''',
    '''                "chapter": metadata.chapter,\n                "order": metadata.order,\n                "status": metadata.status,\n                "pov": metadata.pov,\n                "location": metadata.location,\n                "timeline": metadata.timeline,\n                "tags": metadata.tags,\n                "summary": metadata.summary\n''',
    1,
)
# test literal gets defaults for new fields
s = s.replace(
    '''                status: "draft".into(),\n                page: PageProfile::default(),\n''',
    '''                status: "draft".into(),\n                pov: String::new(),\n                location: String::new(),\n                timeline: String::new(),\n                tags: Vec::new(),\n                summary: String::new(),\n                page: PageProfile::default(),\n''',
    1,
)
# add update helpers before set_page_profile
anchor = '''pub fn set_page_profile(path: &Path, page: PageProfile) -> Result<()> {\n'''
helpers = r'''pub fn set_status(path: &Path, status: &str) -> Result<()> {
    if !is_native_path(path) {
        return Ok(());
    }
    let mut document = read(path)?;
    let next = match status.trim().to_ascii_lowercase().as_str() {
        "revision" | "revisión" => "revision",
        "final" | "finished" | "terminado" => "final",
        _ => "draft",
    };
    if document.metadata.status == next {
        return Ok(());
    }
    document.metadata.status = next.into();
    let project = document.metadata.project.clone();
    write(path, &document)?;
    if !project.trim().is_empty() {
        if let Some(root) = path.parent() {
            sync_project_manifest(root, &project)?;
        }
    }
    Ok(())
}

pub fn set_story_metadata(
    path: &Path,
    pov: &str,
    location: &str,
    timeline: &str,
    tags: &[String],
    summary: &str,
) -> Result<()> {
    if !is_native_path(path) {
        return Ok(());
    }
    let mut document = read(path)?;
    document.metadata.pov = pov.trim().to_owned();
    document.metadata.location = location.trim().to_owned();
    document.metadata.timeline = timeline.trim().to_owned();
    document.metadata.tags = tags
        .iter()
        .map(|tag| tag.trim())
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect();
    document.metadata.summary = summary.trim().to_owned();
    let project = document.metadata.project.clone();
    write(path, &document)?;
    if !project.trim().is_empty() {
        if let Some(root) = path.parent() {
            sync_project_manifest(root, &project)?;
        }
    }
    Ok(())
}

'''
if anchor not in s:
    raise SystemExit("missing set_page_profile anchor")
s = s.replace(anchor, helpers + anchor, 1)
p.write_text(s, encoding="utf-8")

# ---------- transient manuscript navigator + metadata editor ----------
p = Path("src/app.rs")
s = p.read_text(encoding="utf-8")
style_anchor = '''impl StyleSnapshot {\n'''
structs = r'''#[derive(Clone, Debug)]
struct ManuscriptEntry {
    path: PathBuf,
    metadata: DocumentMetadata,
    words: usize,
}

#[derive(Clone, Debug)]
struct ManuscriptNavigator {
    project: String,
    entries: Vec<ManuscriptEntry>,
    selected: usize,
}

#[derive(Clone, Debug)]
struct StoryMetadataEditor {
    path: PathBuf,
    metadata: DocumentMetadata,
    field: usize,
    editing: bool,
    buffer: String,
}

'''
if style_anchor not in s:
    raise SystemExit("missing StyleSnapshot impl anchor")
s = s.replace(style_anchor, structs + style_anchor, 1)

s = s.replace(
    '''    closed_tabs: Vec<PathBuf>,\n}\n''',
    '''    closed_tabs: Vec<PathBuf>,\n    manuscript_nav: Option<ManuscriptNavigator>,\n    metadata_editor: Option<StoryMetadataEditor>,\n}\n''',
    1,
)
s = s.replace(
    '''            closed_tabs: Vec::new(),\n        };\n''',
    '''            closed_tabs: Vec::new(),\n            manuscript_nav: None,\n            metadata_editor: None,\n        };\n''',
    1,
)

# helpers before insert_symbol
marker = '    fn insert_symbol(&mut self, symbol: &str) {'
helpers = r'''    fn overlay_active(&self) -> bool {
        self.manuscript_nav.is_some() || self.metadata_editor.is_some()
    }

    fn status_label(status: &str) -> &'static str {
        match status.to_ascii_lowercase().as_str() {
            "revision" | "revisión" => "REVISIÓN",
            "final" => "FINAL",
            _ => "BORRADOR",
        }
    }

    fn next_status(status: &str) -> &'static str {
        match status.to_ascii_lowercase().as_str() {
            "draft" | "borrador" => "revision",
            "revision" | "revisión" => "final",
            _ => "draft",
        }
    }

    fn open_manuscript_navigator(&mut self) {
        let Some(current) = self.current_file.clone() else {
            return;
        };
        let Some(metadata) = document::read_metadata(&current) else {
            self.launcher.message = Some("Este documento no tiene metadatos de proyecto.".into());
            return;
        };
        if metadata.project.trim().is_empty() {
            self.launcher.message = Some("Asigna el documento a un proyecto desde el launcher (P).".into());
            return;
        }
        let Ok(documents) = document::project_documents(&self.library.documents, &metadata.project) else {
            self.launcher.message = Some("No se pudo leer el manuscrito del proyecto.".into());
            return;
        };
        let entries = documents
            .into_iter()
            .map(|(path, metadata)| {
                let words = document::read(&path)
                    .map(|doc| doc.body.split_whitespace().count())
                    .unwrap_or(0);
                ManuscriptEntry { path, metadata, words }
            })
            .collect::<Vec<_>>();
        let selected = entries
            .iter()
            .position(|entry| paths_equivalent(&entry.path, &current))
            .unwrap_or(0);
        self.manuscript_nav = Some(ManuscriptNavigator {
            project: metadata.project,
            entries,
            selected,
        });
        self.metadata_editor = None;
        self.render_manuscript_navigator();
    }

    fn render_manuscript_navigator(&mut self) {
        let Some(nav) = self.manuscript_nav.as_ref() else {
            return;
        };
        let (cols, rows) = self.terminal_size();
        let width = cols as usize;
        let total_words = nav.entries.iter().map(|entry| entry.words).sum::<usize>();
        let mut out = String::from("\x1b[2J\x1b[H\x1b[?25l");
        push_line(
            &mut out,
            &format!(
                "\x1b[1;38;5;222mMANUSCRITO\x1b[0m  \x1b[38;5;250m{}\x1b[0m",
                truncate(&nav.project, width.saturating_sub(14))
            ),
        );
        push_line(
            &mut out,
            &format!(
                "\x1b[38;5;109m{} documentos · {} palabras\x1b[0m",
                nav.entries.len(), total_words
            ),
        );
        push_line(&mut out, "");

        let available = rows.saturating_sub(7) as usize;
        let start = nav.selected.saturating_sub(available.saturating_sub(1) / 2);
        let end = (start + available).min(nav.entries.len());
        for (index, entry) in nav.entries.iter().enumerate().take(end).skip(start) {
            let selected = index == nav.selected;
            let marker = if selected { "▶" } else { " " };
            let chapter = entry
                .metadata
                .chapter
                .map(|value| format!("{value:02}"))
                .unwrap_or_else(|| "--".into());
            let status = Self::status_label(&entry.metadata.status);
            let title_width = width.saturating_sub(35).max(12);
            let title = truncate(&entry.metadata.title, title_width);
            let line = format!(
                "{marker} {chapter}  {title:<title_width$}  {:>6} pal  {status}",
                entry.words
            );
            if selected {
                push_line(&mut out, &format!("\x1b[30;48;5;109m{}\x1b[0m", truncate(&line, width)));
            } else {
                push_line(&mut out, &format!("\x1b[38;5;250m{}\x1b[0m", truncate(&line, width)));
            }
        }
        while out.bytes().filter(|byte| *byte == b'\n').count() < rows.saturating_sub(2) as usize {
            push_line(&mut out, "");
        }
        push_line(
            &mut out,
            "\x1b[38;5;244m↑↓ mover · Enter abrir pestaña · I ficha · S estado · Esc cerrar\x1b[0m",
        );
        self.reset_parser();
        self.parser.process(out.as_bytes());
        self.glyphs.clear();
        self.dirty = true;
    }

    fn close_workspace_overlay(&mut self) {
        self.manuscript_nav = None;
        self.metadata_editor = None;
        self.reset_parser();
        if let Some(editor) = self.editor.as_mut() {
            let _ = editor.force_terminal_redraw();
        }
        self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(150));
        self.glyphs.clear();
        self.dirty = true;
    }

    fn manuscript_key(&mut self, key: KeyEvent) {
        let Some(nav) = self.manuscript_nav.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.close_workspace_overlay();
                return;
            }
            KeyCode::Up => nav.selected = nav.selected.saturating_sub(1),
            KeyCode::Down => {
                nav.selected = (nav.selected + 1).min(nav.entries.len().saturating_sub(1));
            }
            KeyCode::Enter => {
                let path = nav.entries.get(nav.selected).map(|entry| entry.path.clone());
                self.manuscript_nav = None;
                if let Some(path) = path {
                    if let Err(error) = self.open_in_tab(path) {
                        self.launcher.message = Some(format!("No se pudo abrir el documento: {error}"));
                    }
                }
                return;
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if let Some(entry) = nav.entries.get_mut(nav.selected) {
                    let next = Self::next_status(&entry.metadata.status);
                    if document::set_status(&entry.path, next).is_ok() {
                        entry.metadata.status = next.into();
                    }
                }
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                let path = nav.entries.get(nav.selected).map(|entry| entry.path.clone());
                if let Some(path) = path {
                    self.open_story_metadata_editor(path);
                }
                return;
            }
            _ => {}
        }
        self.render_manuscript_navigator();
    }

    fn metadata_field_value(metadata: &DocumentMetadata, field: usize) -> String {
        match field {
            0 => metadata.pov.clone(),
            1 => metadata.location.clone(),
            2 => metadata.timeline.clone(),
            3 => metadata.tags.join(", "),
            _ => metadata.summary.clone(),
        }
    }

    fn open_story_metadata_editor(&mut self, path: PathBuf) {
        let Some(metadata) = document::read_metadata(&path) else {
            return;
        };
        self.manuscript_nav = None;
        self.metadata_editor = Some(StoryMetadataEditor {
            path,
            metadata,
            field: 0,
            editing: false,
            buffer: String::new(),
        });
        self.render_story_metadata_editor();
    }

    fn persist_story_metadata(editor: &StoryMetadataEditor) -> Result<()> {
        document::set_story_metadata(
            &editor.path,
            &editor.metadata.pov,
            &editor.metadata.location,
            &editor.metadata.timeline,
            &editor.metadata.tags,
            &editor.metadata.summary,
        )
    }

    fn render_story_metadata_editor(&mut self) {
        let Some(editor) = self.metadata_editor.as_ref() else {
            return;
        };
        let (cols, rows) = self.terminal_size();
        let width = cols as usize;
        let mut out = String::from("\x1b[2J\x1b[H\x1b[?25l");
        push_line(
            &mut out,
            &format!(
                "\x1b[1;38;5;222mFICHA\x1b[0m  \x1b[38;5;250m{}\x1b[0m",
                truncate(&editor.metadata.title, width.saturating_sub(9))
            ),
        );
        push_line(
            &mut out,
            &format!(
                "\x1b[38;5;109m{} · {}\x1b[0m",
                editor.metadata.project,
                Self::status_label(&editor.metadata.status)
            ),
        );
        push_line(&mut out, "");
        let labels = ["POV", "Ubicación", "Tiempo", "Etiquetas", "Resumen"];
        for (index, label) in labels.iter().enumerate() {
            let selected = index == editor.field;
            let value = if selected && editor.editing {
                format!("{}▏", editor.buffer)
            } else {
                Self::metadata_field_value(&editor.metadata, index)
            };
            let line = format!("{:>10}  {}", label, truncate(&value, width.saturating_sub(14)));
            if selected {
                push_line(&mut out, &format!("\x1b[30;48;5;109m{}\x1b[0m", truncate(&line, width)));
            } else {
                push_line(&mut out, &format!("\x1b[38;5;250m{}\x1b[0m", truncate(&line, width)));
            }
        }
        while out.bytes().filter(|byte| *byte == b'\n').count() < rows.saturating_sub(2) as usize {
            push_line(&mut out, "");
        }
        let help = if editor.editing {
            "Enter guardar · Esc cancelar · escribir para editar"
        } else {
            "↑↓ campo · Enter editar · S estado · M manuscrito · Esc volver"
        };
        push_line(&mut out, &format!("\x1b[38;5;244m{help}\x1b[0m"));
        self.reset_parser();
        self.parser.process(out.as_bytes());
        self.glyphs.clear();
        self.dirty = true;
    }

    fn story_metadata_key(&mut self, key: KeyEvent) {
        let Some(editor) = self.metadata_editor.as_mut() else {
            return;
        };
        if editor.editing {
            match key.code {
                KeyCode::Esc => {
                    editor.editing = false;
                    editor.buffer.clear();
                }
                KeyCode::Enter => {
                    let value = editor.buffer.trim().to_owned();
                    match editor.field {
                        0 => editor.metadata.pov = value,
                        1 => editor.metadata.location = value,
                        2 => editor.metadata.timeline = value,
                        3 => {
                            editor.metadata.tags = value
                                .split(',')
                                .map(str::trim)
                                .filter(|tag| !tag.is_empty())
                                .map(str::to_owned)
                                .collect();
                        }
                        _ => editor.metadata.summary = value,
                    }
                    editor.editing = false;
                    editor.buffer.clear();
                    let snapshot = editor.clone();
                    if let Err(error) = Self::persist_story_metadata(&snapshot) {
                        self.launcher.message = Some(format!("No se pudo guardar la ficha: {error}"));
                    }
                }
                KeyCode::Backspace => {
                    editor.buffer.pop();
                }
                KeyCode::Char(ch)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    editor.buffer.push(ch);
                }
                _ => {}
            }
            self.render_story_metadata_editor();
            return;
        }

        match key.code {
            KeyCode::Esc => {
                self.metadata_editor = None;
                self.open_manuscript_navigator();
                if self.manuscript_nav.is_none() {
                    self.close_workspace_overlay();
                }
                return;
            }
            KeyCode::Up => editor.field = editor.field.saturating_sub(1),
            KeyCode::Down | KeyCode::Tab => editor.field = (editor.field + 1).min(4),
            KeyCode::Enter => {
                editor.buffer = Self::metadata_field_value(&editor.metadata, editor.field);
                editor.editing = true;
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                let next = Self::next_status(&editor.metadata.status);
                if document::set_status(&editor.path, next).is_ok() {
                    editor.metadata.status = next.into();
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.metadata_editor = None;
                self.open_manuscript_navigator();
                return;
            }
            _ => {}
        }
        self.render_story_metadata_editor();
    }

'''
if marker not in s:
    raise SystemExit("missing insert_symbol marker")
s = s.replace(marker, helpers + marker, 1)

# tick: keep draining Helix output but do not paint over a transient workspace overlay
old = '''                    Ok(bytes) => {\n                        let bytes = editor.normalize_output(&bytes);\n                        self.parser.process(&bytes);\n                        self.dirty = true;\n                    }\n'''
new = '''                    Ok(bytes) => {\n                        let bytes = editor.normalize_output(&bytes);\n                        if !self.overlay_active() {\n                            self.parser.process(&bytes);\n                            self.dirty = true;\n                        }\n                    }\n'''
if old not in s:
    raise SystemExit("missing output processing block")
s = s.replace(old, new, 1)

# Clear overlays when editor exits
s = s.replace(
    '''            self.tabs.clear();\n            self.active_tab = 0;\n            self.launcher.refresh();\n''',
    '''            self.tabs.clear();\n            self.active_tab = 0;\n            self.manuscript_nav = None;\n            self.metadata_editor = None;\n            self.launcher.refresh();\n''',
    1,
)

# Intercept overlays and Ctrl+M at top of editor_key, before F2 state logic.
needle = '''    fn editor_key(&mut self, key: KeyEvent) {\n        // F2 is a transient Helix code-action menu. Once the user confirms an\n'''
insert = '''    fn editor_key(&mut self, key: KeyEvent) {\n        if self.metadata_editor.is_some() {\n            self.story_metadata_key(key);\n            return;\n        }\n        if self.manuscript_nav.is_some() {\n            self.manuscript_key(key);\n            return;\n        }\n        if key.modifiers.contains(KeyModifiers::CONTROL)\n            && !key.modifiers.contains(KeyModifiers::ALT)\n            && matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M'))\n        {\n            self.open_manuscript_navigator();\n            return;\n        }\n\n        // F2 is a transient Helix code-action menu. Once the user confirms an\n'''
if needle not in s:
    raise SystemExit("missing editor_key anchor")
s = s.replace(needle, insert, 1)
p.write_text(s, encoding="utf-8")

# ---------- docs ----------
readme = Path("README.md")
rs = readme.read_text(encoding="utf-8")
row = '| `Ctrl+Shift+T` | reabrir última pestaña cerrada |'
addition = row + '\n| `Ctrl+M` | abrir/cerrar navegador del manuscrito |'
if row in rs and 'navegador del manuscrito' not in rs:
    rs = rs.replace(row, addition, 1)
section = '''\n## Manuscrito sin distracciones\n\n`Ctrl+M` abre una vista transitoria del proyecto actual. Muestra capítulos/documentos, palabras y estado sin dejar un panel permanente en pantalla. `Enter` abre el elemento en una pestaña, `S` alterna Borrador/Revisión/Final e `I` abre su ficha literaria. La ficha guarda POV, ubicación, tiempo, etiquetas y resumen en el sidecar/metadatos del documento. `Esc` vuelve inmediatamente al texto y fuerza un repintado limpio de Helix.\n'''
if '## Manuscrito sin distracciones' not in rs:
    rs = rs.replace('\n## Abrir directamente\n', section + '\n## Abrir directamente\n', 1)
readme.write_text(rs, encoding="utf-8")

ch = Path("CHANGELOG.md")
cs = ch.read_text(encoding="utf-8")
entry = '''## v0.2.23 — 2026-10-17
- Navegador transitorio de manuscrito con Ctrl+M, sin panel lateral permanente.
- Apertura de capítulos y documentos del proyecto directamente en pestañas.
- Recuento de palabras y estado Borrador/Revisión/Final por documento.
- Ficha literaria transitoria con POV, ubicación, tiempo, etiquetas y resumen.
- Metadatos literarios persistentes en sidecar/HSST y manifiesto de proyecto.

'''
marker = '## v0.2.22 — 2026-10-17\n'
if marker in cs and '## v0.2.23' not in cs:
    cs = cs.replace(marker, entry + marker, 1)
ch.write_text(cs, encoding="utf-8")
