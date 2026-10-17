from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    text = p.read_text(encoding='utf-8')
    if old not in text:
        raise SystemExit(f'missing pattern in {path}: {old[:180]!r}')
    p.write_text(text.replace(old, new, 1), encoding='utf-8')

# ---- world.rs: generic record/singleton editing ----
replace_once(
    'src/world.rs',
    '''    pub fn add_record(&mut self, section: WorldSection, title: &str) -> Result<()> {\n''',
    '''    pub fn record_fields(&self, section: WorldSection, index: usize) -> Vec<WorldRow> {\n        if section.is_record_list() {\n            return self.array(section).get(index).map(|value| object_rows(Some(value))).unwrap_or_default();\n        }\n        match section {\n            WorldSection::Project => object_rows(self.root.get("profile")),\n            WorldSection::Layout => object_rows(self.root.get("layout")),\n            WorldSection::Settings => object_rows(self.root.get("settings")),\n            _ => Vec::new(),\n        }\n    }\n\n    pub fn set_field(&mut self, section: WorldSection, index: usize, key: &str, raw: &str) -> Result<bool> {\n        let target = if section.is_record_list() {\n            self.array_mut(section).get_mut(index)\n        } else {\n            match section {\n                WorldSection::Project => self.root.get_mut("profile"),\n                WorldSection::Layout => self.root.get_mut("layout"),\n                WorldSection::Settings => self.root.get_mut("settings"),\n                _ => None,\n            }\n        };\n        let Some(object) = target.and_then(Value::as_object_mut) else { return Ok(false); };\n        let existing = object.get(key).cloned().unwrap_or(Value::String(String::new()));\n        let value = parse_like(&existing, raw);\n        object.insert(key.to_owned(), value);\n        self.touch();\n        self.save()?;\n        Ok(true)\n    }\n\n    pub fn add_record(&mut self, section: WorldSection, title: &str) -> Result<()> {\n'''
)

replace_once(
    'src/world.rs',
    '''fn scalar_text(value: &Value) -> String {\n''',
    '''fn parse_like(existing: &Value, raw: &str) -> Value {\n    match existing {\n        Value::Bool(_) => Value::Bool(matches!(raw.trim().to_ascii_lowercase().as_str(), "1" | "true" | "sí" | "si" | "yes" | "on")),\n        Value::Number(number) if number.is_i64() => raw.trim().parse::<i64>().map(Value::from).unwrap_or_else(|_| existing.clone()),\n        Value::Number(number) if number.is_u64() => raw.trim().parse::<u64>().map(Value::from).unwrap_or_else(|_| existing.clone()),\n        Value::Number(_) => raw.trim().parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number).unwrap_or_else(|| existing.clone()),\n        Value::Array(_) => Value::Array(raw.split(',').map(str::trim).filter(|v| !v.is_empty()).map(|v| Value::String(v.to_owned())).collect()),\n        Value::Null if raw.trim().is_empty() => Value::Null,\n        Value::Null => Value::String(raw.to_owned()),\n        Value::Object(_) => existing.clone(),\n        Value::String(_) => Value::String(raw.to_owned()),\n    }\n}\n\nfn scalar_text(value: &Value) -> String {\n'''
)

# ---- app.rs: richer WBW editor state ----
replace_once(
    'src/app.rs',
    '''    workspace_new: bool,\n    workspace_input: String,\n    message: Option<String>,\n''',
    '''    workspace_new: bool,\n    workspace_input: String,\n    workspace_edit_index: Option<usize>,\n    workspace_record_open: bool,\n    workspace_field: usize,\n    workspace_edit_field: Option<String>,\n    message: Option<String>,\n'''
)
replace_once(
    'src/app.rs',
    '''            workspace_new: false,\n            workspace_input: String::new(),\n            message: None,\n''',
    '''            workspace_new: false,\n            workspace_input: String::new(),\n            workspace_edit_index: None,\n            workspace_record_open: false,\n            workspace_field: 0,\n            workspace_edit_field: None,\n            message: None,\n'''
)

start = '''        if let Some(project) = self.launcher.workspace_project.clone() {\n'''
end = '''        if let Some(path) = self.launcher.project_editing.clone() {\n'''
p = Path('src/app.rs')
text = p.read_text(encoding='utf-8')
a = text.index(start)
b = text.index(end, a)
new_block = r'''        if let Some(project) = self.launcher.workspace_project.clone() {
            let section = WorldSection::ALL[self.launcher.workspace_selected.min(WorldSection::ALL.len() - 1)];

            if let Some(field) = self.launcher.workspace_edit_field.clone() {
                match key.code {
                    KeyCode::Esc => {
                        self.launcher.workspace_edit_field = None;
                        self.launcher.workspace_input.clear();
                    }
                    KeyCode::Enter => {
                        let value = self.launcher.workspace_input.clone();
                        let record = self.launcher.workspace_edit_index.unwrap_or(self.launcher.workspace_row);
                        match world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                            .and_then(|mut workspace| workspace.set_field(section, record, &field, &value))
                        {
                            Ok(true) => self.launcher.message = Some(format!("{} actualizado", field)),
                            Ok(false) => self.launcher.message = Some("Campo no editable.".into()),
                            Err(error) => self.launcher.message = Some(format!("No se pudo guardar WBW: {error}")),
                        }
                        self.launcher.workspace_edit_field = None;
                        self.launcher.workspace_input.clear();
                    }
                    KeyCode::Backspace => { self.launcher.workspace_input.pop(); }
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
                        self.launcher.workspace_input.push(ch);
                    }
                    _ => {}
                }
                self.render_launcher();
                return;
            }

            if self.launcher.workspace_new {
                match key.code {
                    KeyCode::Esc => {
                        self.launcher.workspace_new = false;
                        self.launcher.workspace_input.clear();
                    }
                    KeyCode::Enter => {
                        let title = self.launcher.workspace_input.trim().to_owned();
                        if !title.is_empty() {
                            match world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                                .and_then(|mut workspace| workspace.add_record(section, &title))
                            {
                                Ok(()) => self.launcher.message = Some(format!("{} añadido a {}", title, section.label())),
                                Err(error) => self.launcher.message = Some(format!("No se pudo guardar WBW: {error}")),
                            }
                        }
                        self.launcher.workspace_new = false;
                        self.launcher.workspace_input.clear();
                    }
                    KeyCode::Backspace => { self.launcher.workspace_input.pop(); }
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
                        self.launcher.workspace_input.push(ch);
                    }
                    _ => {}
                }
                self.render_launcher();
                return;
            }

            if self.launcher.workspace_record_open {
                if let Ok(workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                    let record = self.launcher.workspace_edit_index.unwrap_or(self.launcher.workspace_row);
                    let fields = workspace.record_fields(section, record);
                    match key.code {
                        KeyCode::Esc => {
                            self.launcher.workspace_record_open = false;
                            self.launcher.workspace_field = 0;
                            self.launcher.workspace_edit_index = None;
                        }
                        KeyCode::Up => self.launcher.workspace_field = self.launcher.workspace_field.saturating_sub(1),
                        KeyCode::Down => self.launcher.workspace_field = (self.launcher.workspace_field + 1).min(fields.len().saturating_sub(1)),
                        KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('E') => {
                            if let Some(row) = fields.get(self.launcher.workspace_field) {
                                self.launcher.workspace_edit_field = Some(row.label.clone());
                                self.launcher.workspace_input = row.detail.clone();
                            }
                        }
                        _ => {}
                    }
                }
                self.render_launcher();
                return;
            }

            match key.code {
                KeyCode::Esc => {
                    if self.launcher.workspace_detail {
                        self.launcher.workspace_detail = false;
                        self.launcher.workspace_row = 0;
                    } else {
                        self.launcher.workspace_project = None;
                        self.launcher.workspace_selected = 0;
                    }
                }
                KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) && self.launcher.workspace_detail && section.is_record_list() => {
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        if let Ok(index) = workspace.move_record(section, self.launcher.workspace_row, -1) { self.launcher.workspace_row = index; }
                    }
                }
                KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) && self.launcher.workspace_detail && section.is_record_list() => {
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        if let Ok(index) = workspace.move_record(section, self.launcher.workspace_row, 1) { self.launcher.workspace_row = index; }
                    }
                }
                KeyCode::Up => {
                    if self.launcher.workspace_detail {
                        self.launcher.workspace_row = self.launcher.workspace_row.saturating_sub(1);
                    } else {
                        self.launcher.workspace_selected = self.launcher.workspace_selected.saturating_sub(1);
                    }
                }
                KeyCode::Down => {
                    if self.launcher.workspace_detail {
                        if let Ok(workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                            let max = workspace.rows(section).len().saturating_sub(1);
                            self.launcher.workspace_row = (self.launcher.workspace_row + 1).min(max);
                        }
                    } else {
                        self.launcher.workspace_selected = (self.launcher.workspace_selected + 1).min(WorldSection::ALL.len() - 1);
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') if self.launcher.workspace_detail && section.is_record_list() => {
                    self.launcher.workspace_new = true;
                    self.launcher.workspace_input.clear();
                    self.launcher.message = None;
                }
                KeyCode::Char('d') | KeyCode::Char('D') if self.launcher.workspace_detail && section.is_record_list() => {
                    if let Ok(mut workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        match workspace.delete_record(section, self.launcher.workspace_row) {
                            Ok(true) => {
                                self.launcher.message = Some("Registro eliminado.".into());
                                self.launcher.workspace_row = self.launcher.workspace_row.saturating_sub(1);
                            }
                            Ok(false) => {}
                            Err(error) => self.launcher.message = Some(format!("No se pudo eliminar: {error}")),
                        }
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') if self.launcher.workspace_detail && section.is_record_list() => {
                    if let Ok(workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                        if let Some(row) = workspace.rows(section).get(self.launcher.workspace_row) {
                            self.launcher.workspace_edit_index = Some(self.launcher.workspace_row);
                            let fields = workspace.record_fields(section, self.launcher.workspace_row);
                            let title_field = fields.iter().find(|field| field.label == "title").or_else(|| fields.iter().find(|field| field.label == "name"));
                            if let Some(field) = title_field {
                                self.launcher.workspace_edit_field = Some(field.label.clone());
                                self.launcher.workspace_input = row.label.clone();
                            }
                        }
                    }
                }
                KeyCode::Char('a') | KeyCode::Char('A') if self.launcher.workspace_detail && section == WorldSection::Media => {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Multimedia", &["mp3","mp4","ogg","vob","wav","flac","m4a","webm","mov","avi","mkv"]).pick_file() {
                        let title = path.file_stem().and_then(|v| v.to_str()).unwrap_or("Medio").to_owned();
                        match world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                            .and_then(|mut workspace| workspace.add_media(&title, &path))
                        {
                            Ok(()) => self.launcher.message = Some(format!("Medio añadido: {title}")),
                            Err(error) => self.launcher.message = Some(format!("No se pudo añadir medio: {error}")),
                        }
                    }
                }
                KeyCode::Char('b') | KeyCode::Char('B') if !self.launcher.workspace_detail => {
                    match world::ProjectWorkspace::load_or_create(&self.library.documents, &project)
                        .and_then(|workspace| workspace.export_package(&self.library.documents, &self.library.exports))
                    {
                        Ok(path) => self.launcher.message = Some(format!("Paquete WBW: {}", path.display())),
                        Err(error) => self.launcher.message = Some(format!("No se pudo exportar WBW: {error}")),
                    }
                }
                KeyCode::Char('i') | KeyCode::Char('I') if !self.launcher.workspace_detail => {
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
                    if self.launcher.workspace_detail {
                        if let Ok(workspace) = world::ProjectWorkspace::load_or_create(&self.library.documents, &project) {
                            let rows = workspace.rows(section);
                            if let Some(path) = rows.get(self.launcher.workspace_row).and_then(|row| row.path.clone()).filter(|path| path.is_file()) {
                                self.launcher.workspace_project = None;
                                self.launcher.workspace_detail = false;
                                self.launcher.workspace_row = 0;
                                if let Err(error) = self.open_editor(path) { self.launcher.message = Some(error.to_string()); }
                                return;
                            }
                            if section.is_record_list() && self.launcher.workspace_row < rows.len() {
                                self.launcher.workspace_record_open = true;
                                self.launcher.workspace_edit_index = Some(self.launcher.workspace_row);
                                self.launcher.workspace_field = 0;
                            } else if matches!(section, WorldSection::Project | WorldSection::Layout | WorldSection::Settings) {
                                if let Some(row) = rows.get(self.launcher.workspace_row) {
                                    self.launcher.workspace_edit_field = Some(row.label.clone());
                                    self.launcher.workspace_input = row.detail.clone();
                                }
                            }
                        }
                    } else {
                        self.launcher.workspace_detail = true;
                        self.launcher.workspace_row = 0;
                    }
                }
                _ => {}
            }
            if self.editor.is_none() { self.render_launcher(); }
            return;
        }

'''
text = text[:a] + new_block + text[b:]
p.write_text(text, encoding='utf-8')

# Render workspace states.
p = Path('src/app.rs')
text = p.read_text(encoding='utf-8')
old = r'''                    if self.launcher.workspace_new {
                        push_line(&mut out, &framed_left(
                            &format!("Nuevo {}: {}\x1b[?25h", WorldSection::ALL[self.launcher.workspace_selected].label(), self.launcher.workspace_input),
                            inner_width, "38;5;244"));
                        push_line(&mut out, &framed_left("Enter guardar · Esc cancelar", inner_width, "38;5;244"));
                    } else if self.launcher.workspace_detail {
                        let section = WorldSection::ALL[self.launcher.workspace_selected];
                        push_line(&mut out, &framed_left(
                            &format!("\x1b[38;5;109m{}\x1b[0m", section.label()), inner_width, "38;5;244"));
                        let rows = workspace.rows(section);
                        if rows.is_empty() {
                            push_line(&mut out, &framed_left("(sin registros)", inner_width, "38;5;244"));
                        } else {
                            for (index, row) in rows.iter().enumerate().take((rows.len()).min(12)) {
                                let marker = if index == self.launcher.workspace_row { "▶" } else { " " };
                                let detail = if row.detail.is_empty() { String::new() } else { format!(" · {}", row.detail) };
                                let line = format!("{marker} {}{}", row.label, detail);
                                push_line(&mut out, &framed_left(&truncate(&line, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                            }
                        }
                        if !matches!(section, WorldSection::Manuscript | WorldSection::Statistics) {
                            push_line(&mut out, &framed_left("N nuevo · Enter abrir · Esc volver", inner_width, "38;5;244"));
                        } else {
                            push_line(&mut out, &framed_left("Enter abrir · Esc volver", inner_width, "38;5;244"));
                        }
                    } else {
'''
new = r'''                    if let Some(field) = self.launcher.workspace_edit_field.as_ref() {
                        push_line(&mut out, &framed_left(
                            &format!("Editar {field}: {}\x1b[?25h", self.launcher.workspace_input), inner_width, "38;5;244"));
                        push_line(&mut out, &framed_left("Enter guardar · Esc cancelar", inner_width, "38;5;244"));
                    } else if self.launcher.workspace_new {
                        push_line(&mut out, &framed_left(
                            &format!("Nuevo {}: {}\x1b[?25h", WorldSection::ALL[self.launcher.workspace_selected].label(), self.launcher.workspace_input),
                            inner_width, "38;5;244"));
                        push_line(&mut out, &framed_left("Enter guardar · Esc cancelar", inner_width, "38;5;244"));
                    } else if self.launcher.workspace_record_open {
                        let section = WorldSection::ALL[self.launcher.workspace_selected];
                        let record = self.launcher.workspace_edit_index.unwrap_or(self.launcher.workspace_row);
                        let fields = workspace.record_fields(section, record);
                        push_line(&mut out, &framed_left(&format!("\x1b[38;5;109m{} · ficha\x1b[0m", section.label()), inner_width, "38;5;244"));
                        for (index, row) in fields.iter().enumerate().take(14) {
                            let marker = if index == self.launcher.workspace_field { "▶" } else { " " };
                            let line = format!("{marker} {}: {}", row.label, row.detail);
                            push_line(&mut out, &framed_left(&truncate(&line, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                        }
                        push_line(&mut out, &framed_left("Enter/E editar campo · Esc volver", inner_width, "38;5;244"));
                    } else if self.launcher.workspace_detail {
                        let section = WorldSection::ALL[self.launcher.workspace_selected];
                        push_line(&mut out, &framed_left(
                            &format!("\x1b[38;5;109m{}\x1b[0m", section.label()), inner_width, "38;5;244"));
                        let rows = workspace.rows(section);
                        if rows.is_empty() {
                            push_line(&mut out, &framed_left("(sin registros)", inner_width, "38;5;244"));
                        } else {
                            for (index, row) in rows.iter().enumerate().take((rows.len()).min(14)) {
                                let marker = if index == self.launcher.workspace_row { "▶" } else { " " };
                                let detail = if row.detail.is_empty() { String::new() } else { format!(" · {}", row.detail) };
                                let line = format!("{marker} {}{}", row.label, detail);
                                push_line(&mut out, &framed_left(&truncate(&line, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                            }
                        }
                        if section.is_record_list() {
                            let extra = if section == WorldSection::Media { " · A adjuntar" } else { "" };
                            push_line(&mut out, &framed_left(&format!("N nuevo · Enter ficha · R renombrar · D eliminar · Ctrl+↑↓ mover{extra} · Esc volver"), inner_width, "38;5;244"));
                        } else if matches!(section, WorldSection::Project | WorldSection::Layout | WorldSection::Settings) {
                            push_line(&mut out, &framed_left("Enter editar campo · Esc volver", inner_width, "38;5;244"));
                        } else {
                            push_line(&mut out, &framed_left("Enter abrir · Esc volver", inner_width, "38;5;244"));
                        }
                    } else {
'''
if old not in text:
    raise SystemExit('workspace render block not found')
text = text.replace(old, new, 1)
text = text.replace('↑↓ seleccionar · Enter abrir · Esc archivos', '↑↓ seleccionar · Enter abrir · B exportar .wbw · I importar .wbw · Esc archivos', 1)
p.write_text(text, encoding='utf-8')

# Update changelog and README instructions.
ch = Path('CHANGELOG.md')
log = ch.read_text(encoding='utf-8')
entry = '''## WBW complete workspace — 2026-10-17\n\n- Completado el workspace WBW: Proyecto, Manuscrito, Tablero de escenas, Personajes, Relaciones, Hilos/teorías, Cronología, Atlas, Mapas, Magia, Referencias, Revisión, Maquetación, Multimedia, Configuración y Estadísticas.\n- Las fichas permiten editar todos sus campos escalares/listas; los registros se crean, renombran, eliminan y reordenan.\n- Multimedia local puede adjuntarse desde el workspace.\n- Importación/exportación `.wbw` portable con `manifest.json`, `project.json`, manuscrito y medios.\n- Proyecto, maquetación y configuración son editables sin contaminar los TXT.\n\n'''
if '## WBW complete workspace — 2026-10-17' not in log:
    marker = log.find('## ')
    log = log[:marker] + entry + log[marker:] if marker >= 0 else entry + log
    ch.write_text(log, encoding='utf-8')

readme = Path('README.md')
rt = readme.read_text(encoding='utf-8')
if '### Workspace WBW integrado' not in rt:
    rt += '''\n\n### Workspace WBW integrado\n\nDesde el navegador, `W` abre el workspace del proyecto seleccionado. Incluye proyecto, manuscrito, escenas, personajes, relaciones, teorías, cronología, atlas, mapas, magia, referencias, revisión, maquetación, multimedia, configuración y estadísticas. `N` crea registros, `Enter` abre/edita, `R` renombra, `D` elimina, `Ctrl+↑/↓` reordena, `A` adjunta multimedia, `B` exporta `.wbw` e `I` importa `.wbw`.\n'''
    readme.write_text(rt, encoding='utf-8')
