from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    text = p.read_text(encoding='utf-8')
    if old not in text:
        raise SystemExit(f'missing pattern in {path}: {old[:180]!r}')
    p.write_text(text.replace(old, new, 1), encoding='utf-8')

# Scene reorder must release the mutable rows borrow before syncing native documents.
replace_once('src/world.rs', '''            if section == WorldSection::Scenes {
                for scene_index in 0..rows.len() {
                    self.sync_scene_document(scene_index)?;
                }
            }
            self.touch();
''', '''            let scene_count = if section == WorldSection::Scenes { rows.len() } else { 0 };
            let _ = rows;
            for scene_index in 0..scene_count {
                self.sync_scene_document(scene_index)?;
            }
            self.touch();
''')

# Rename also updates the native scene TXT metadata.
replace_once('src/world.rs', '''        record.insert(key.into(), json!(title.trim()));
        self.touch();
        self.save()?;
        Ok(true)
''', '''        record.insert(key.into(), json!(title.trim()));
        if section == WorldSection::Scenes {
            self.sync_scene_document(index)?;
        }
        self.touch();
        self.save()?;
        Ok(true)
''')

# Replace package export/import with round-trip-safe portable paths for manuscript, scenes and media.
p = Path('src/world.rs')
text = p.read_text(encoding='utf-8')
start = text.index('    pub fn import_package(package: &Path, documents: &Path) -> Result<Self> {')
end = text.index('    pub fn save(&self) -> Result<()> {', start)
replacement = r'''    pub fn import_package(package: &Path, documents: &Path) -> Result<Self> {
        let file = fs::File::open(package)
            .with_context(|| format!("No se pudo abrir {}", package.display()))?;
        let mut archive = ZipArchive::new(file)
            .with_context(|| format!("{} no es un paquete WBW válido", package.display()))?;
        let mut raw = String::new();
        archive
            .by_name(PACKAGE_PROJECT)
            .context("El paquete no contiene project.json")?
            .read_to_string(&mut raw)?;
        let mut root: Value = serde_json::from_str(&raw).context("project.json inválido")?;
        let name = root
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Proyecto importado")
            .to_owned();

        let mut extracted = std::collections::HashMap::<String, PathBuf>::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            if entry.is_dir() { continue; }
            let entry_name = entry.name().replace('\\', "/");
            let destination = if entry_name.starts_with("manuscript/") {
                let file_name = Path::new(&entry_name).file_name().and_then(|v| v.to_str()).map(safe_component).unwrap_or_else(generate_id);
                Some(documents.join(file_name))
            } else if entry_name.starts_with("media/") {
                let file_name = Path::new(&entry_name).file_name().and_then(|v| v.to_str()).map(safe_component).unwrap_or_else(generate_id);
                Some(documents.join(".zen-media").join(safe_component(&name)).join(file_name))
            } else {
                None
            };
            let Some(target) = destination else { continue; };
            if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            fs::write(&target, bytes)?;
            extracted.insert(entry_name, target);
        }

        if let Some(sections) = root.get_mut("sections").and_then(Value::as_object_mut) {
            for section_name in ["manuscript", "scenes", "media"] {
                if let Some(rows) = sections.get_mut(section_name).and_then(Value::as_array_mut) {
                    for row in rows {
                        let package_path = row.get("package_path").and_then(Value::as_str).map(str::to_owned);
                        if let Some(path) = package_path.as_ref().and_then(|key| extracted.get(key)) {
                            if let Some(object) = row.as_object_mut() {
                                object.insert("path".into(), json!(path.to_string_lossy()));
                            }
                        }
                    }
                }
            }
        }

        let mut workspace = Self {
            name: name.clone(),
            path: project_path(documents, &name),
            documents: documents.to_path_buf(),
            root,
        };
        workspace.normalize();
        workspace.sync_scene_records()?;
        workspace.sync_manuscript(documents)?;
        workspace.save()?;
        Ok(workspace)
    }

    pub fn export_package(&self, _documents: &Path, destination: &Path) -> Result<PathBuf> {
        fs::create_dir_all(destination)?;
        let target = destination.join(format!("{}.wbw", safe_component(&self.name)));
        let file = fs::File::create(&target)?;
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let manifest = json!({
            "format": PROJECT_FORMAT,
            "application": "Zen Editor",
            "project": self.name,
            "project_file": PACKAGE_PROJECT,
            "created": unix_seconds(),
        });

        let mut packaged = self.root.clone();
        let mut files = Vec::<(String, PathBuf)>::new();
        if let Some(sections) = packaged.get_mut("sections").and_then(Value::as_object_mut) {
            for (section_name, prefix) in [("manuscript", "manuscript"), ("scenes", "manuscript"), ("media", "media")] {
                if let Some(rows) = sections.get_mut(section_name).and_then(Value::as_array_mut) {
                    for row in rows {
                        let Some(path) = row.get("path").and_then(Value::as_str).map(PathBuf::from) else { continue; };
                        if !path.is_file() { continue; }
                        let name = path.file_name().and_then(|v| v.to_str()).map(safe_component).unwrap_or_else(generate_id);
                        let package_path = format!("{prefix}/{name}");
                        if let Some(object) = row.as_object_mut() {
                            object.insert("package_path".into(), json!(package_path));
                        }
                        files.push((package_path, path.clone()));
                        if prefix == "manuscript" {
                            let sidecar = path.with_file_name(format!("{}.zen.json", path.file_name().and_then(|v| v.to_str()).unwrap_or("document.txt")));
                            if sidecar.is_file() {
                                let side_name = sidecar.file_name().and_then(|v| v.to_str()).map(safe_component).unwrap_or_else(|| format!("{}.zen.json", generate_id()));
                                files.push((format!("manuscript/{side_name}"), sidecar));
                            }
                        }
                    }
                }
            }
        }

        zip.start_file(PACKAGE_MANIFEST, options)?;
        zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
        zip.start_file(PACKAGE_PROJECT, options)?;
        zip.write_all(serde_json::to_string_pretty(&packaged)?.as_bytes())?;

        let mut included = std::collections::HashSet::new();
        for (package_path, path) in files {
            if included.insert(package_path.clone()) {
                zip.start_file(package_path, options)?;
                zip.write_all(&fs::read(path)?)?;
            }
        }
        zip.finish()?;
        Ok(target)
    }

'''
text = text[:start] + replacement + text[end:]
p.write_text(text, encoding='utf-8')

# Freeze terminal consumption while semantic picker is visible and cleanly restore/redraw before insertion.
replace_once('src/app.rs', '''        self.update_pomodoro();
        let mut finished = false;
''', '''        self.update_pomodoro();
        if self.reference_picker.is_some() {
            return;
        }
        let mut finished = false;
''')
replace_once('src/app.rs', '''                KeyCode::Enter => {
                    if let Some((id, label)) = rows.get(selected) {
                        let marker = format!("[[ref:{id}|{label}]]");
                        self.reference_picker = None;
                        self.insert_symbol(&marker);
                    }
                }
''', '''                KeyCode::Enter => {
                    if let Some((id, label)) = rows.get(selected) {
                        let marker = format!("[[ref:{id}|{label}]]");
                        self.reference_picker = None;
                        self.reset_parser();
                        if let Some(editor) = self.editor.as_mut() { let _ = editor.force_terminal_redraw(); }
                        self.insert_symbol(&marker);
                    }
                }
''')

# Keep the picker visible when navigating it.
replace_once('src/app.rs', '''                KeyCode::Up => { self.reference_picker = Some((project, rows, selected.saturating_sub(1))); }
                KeyCode::Down => { self.reference_picker = Some((project, rows.clone(), (selected + 1).min(rows.len().saturating_sub(1)))); }
''', '''                KeyCode::Up => { self.reference_picker = Some((project, rows, selected.saturating_sub(1))); self.render_reference_picker(); }
                KeyCode::Down => { self.reference_picker = Some((project, rows.clone(), (selected + 1).min(rows.len().saturating_sub(1)))); self.render_reference_picker(); }
''')

p = Path('CHANGELOG.md')
log = p.read_text(encoding='utf-8')
entry = '''## Portable scene round-trip — 2026-10-17\n\n- Exportación/importación `.wbw` ahora conserva y restaura manuscrito, escenas TXT, sidecars de formato y multimedia con rutas portables.\n- Renombrar/reordenar/editar una escena sincroniza sus metadatos nativos; eliminarla retira TXT y sidecar.\n- El selector F3 de referencias semánticas permanece estable mientras está abierto y restaura el terminal antes de insertar el vínculo.\n\n'''
if '## Portable scene round-trip — 2026-10-17' not in log:
    marker = log.find('## ')
    log = log[:marker] + entry + log[marker:] if marker >= 0 else entry + log
    p.write_text(log, encoding='utf-8')
