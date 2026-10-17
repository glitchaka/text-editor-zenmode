use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde_json::{Value, json};
use zip::{ZipArchive, ZipWriter, write::FileOptions};

use crate::document;

const PROJECT_FORMAT: u32 = 2;
const PACKAGE_MANIFEST: &str = "manifest.json";
const PACKAGE_PROJECT: &str = "project.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldSection {
    Project,
    Manuscript,
    Scenes,
    Characters,
    Relationships,
    Theories,
    Timeline,
    World,
    Maps,
    Magic,
    References,
    Notes,
    Review,
    Layout,
    Media,
    Settings,
    Statistics,
}

impl WorldSection {
    pub const ALL: [Self; 17] = [
        Self::Project,
        Self::Manuscript,
        Self::Scenes,
        Self::Characters,
        Self::Relationships,
        Self::Theories,
        Self::Timeline,
        Self::World,
        Self::Maps,
        Self::Magic,
        Self::References,
        Self::Notes,
        Self::Review,
        Self::Layout,
        Self::Media,
        Self::Settings,
        Self::Statistics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Project => "Proyecto",
            Self::Manuscript => "Manuscrito",
            Self::Scenes => "Tablero de escenas",
            Self::Characters => "Personajes",
            Self::Relationships => "Relaciones",
            Self::Theories => "Hilos / teorías",
            Self::Timeline => "Cronología",
            Self::World => "Atlas / mundo",
            Self::Maps => "Mapas",
            Self::Magic => "Sistemas de magia",
            Self::References => "Textos / referencias",
            Self::Notes => "Documentación / notas",
            Self::Review => "Revisión",
            Self::Layout => "Maquetación",
            Self::Media => "Multimedia",
            Self::Settings => "Configuración",
            Self::Statistics => "Estadísticas",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Manuscript => "manuscript",
            Self::Scenes => "scenes",
            Self::Characters => "characters",
            Self::Relationships => "relationships",
            Self::Theories => "theories",
            Self::Timeline => "timeline",
            Self::World => "world",
            Self::Maps => "maps",
            Self::Magic => "magic",
            Self::References => "references",
            Self::Notes => "notes",
            Self::Review => "review",
            Self::Layout => "layout",
            Self::Media => "media",
            Self::Settings => "settings",
            Self::Statistics => "statistics",
        }
    }

    pub fn is_record_list(self) -> bool {
        matches!(
            self,
            Self::Scenes
                | Self::Characters
                | Self::Relationships
                | Self::Theories
                | Self::Timeline
                | Self::World
                | Self::Maps
                | Self::Magic
                | Self::References
                | Self::Notes
                | Self::Review
                | Self::Media
        )
    }
}

#[derive(Clone, Debug)]
pub struct WorldRow {
    pub label: String,
    pub detail: String,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct WorldSummary {
    pub chapters: usize,
    pub scenes: usize,
    pub characters: usize,
    pub relationships: usize,
    pub theories: usize,
    pub timeline: usize,
    pub world: usize,
    pub maps: usize,
    pub magic: usize,
    pub references: usize,
    pub notes: usize,
    pub review: usize,
    pub media: usize,
    pub words: usize,
}

impl WorldSummary {
    pub fn count(&self, section: WorldSection) -> usize {
        match section {
            WorldSection::Project => 1,
            WorldSection::Manuscript => self.chapters,
            WorldSection::Scenes => self.scenes,
            WorldSection::Characters => self.characters,
            WorldSection::Relationships => self.relationships,
            WorldSection::Theories => self.theories,
            WorldSection::Timeline => self.timeline,
            WorldSection::World => self.world,
            WorldSection::Maps => self.maps,
            WorldSection::Magic => self.magic,
            WorldSection::References => self.references,
            WorldSection::Notes => self.notes,
            WorldSection::Review => self.review,
            WorldSection::Layout => 1,
            WorldSection::Media => self.media,
            WorldSection::Settings => 1,
            WorldSection::Statistics => self.words,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProjectWorkspace {
    pub name: String,
    pub path: PathBuf,
    documents: PathBuf,
    root: Value,
}

impl ProjectWorkspace {
    pub fn load_or_create(documents: &Path, project: &str) -> Result<Self> {
        let project = project.trim();
        if project.is_empty() {
            anyhow::bail!("El proyecto no puede estar vacío");
        }
        let path = project_path(documents, project);
        let root = if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("No se pudo leer {}", path.display()))?;
            serde_json::from_str::<Value>(&raw)
                .with_context(|| format!("Proyecto WBW inválido: {}", path.display()))?
        } else {
            empty_project(project)
        };
        let mut workspace = Self {
            name: project.to_owned(),
            path,
            documents: documents.to_path_buf(),
            root,
        };
        workspace.normalize();
        workspace.sync_scene_records()?;
        workspace.sync_manuscript(documents)?;
        workspace.save()?;
        Ok(workspace)
    }

    pub fn import_package(package: &Path, documents: &Path) -> Result<Self> {
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

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(&self.root)?;
        fs::write(&self.path, raw)
            .with_context(|| format!("No se pudo guardar {}", self.path.display()))
    }

    pub fn summary(&self) -> WorldSummary {
        let mut summary = WorldSummary::default();
        summary.chapters = self.array(WorldSection::Manuscript).len();
        summary.scenes = self.array(WorldSection::Scenes).len();
        summary.characters = self.array(WorldSection::Characters).len();
        summary.relationships = self.array(WorldSection::Relationships).len();
        summary.theories = self.array(WorldSection::Theories).len();
        summary.timeline = self.array(WorldSection::Timeline).len();
        summary.world = self.array(WorldSection::World).len();
        summary.maps = self.array(WorldSection::Maps).len();
        summary.magic = self.array(WorldSection::Magic).len();
        summary.references = self.array(WorldSection::References).len();
        summary.notes = self.array(WorldSection::Notes).len();
        summary.review = self.array(WorldSection::Review).len();
        summary.media = self.array(WorldSection::Media).len();
        summary.words = self
            .array(WorldSection::Manuscript)
            .iter()
            .chain(self.array(WorldSection::Scenes).iter())
            .filter_map(|row| row.get("words").and_then(Value::as_u64))
            .map(|value| value as usize)
            .sum();
        summary
    }

    pub fn semantic_references(&self) -> Vec<(String, String)> {
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
        match section {
            WorldSection::Project => return object_rows(self.root.get("profile")),
            WorldSection::Layout => return object_rows(self.root.get("layout")),
            WorldSection::Settings => return object_rows(self.root.get("settings")),
            WorldSection::Statistics => {
                let summary = self.summary();
                return vec![
                    stat_row("Palabras", summary.words),
                    stat_row("Capítulos/documentos", summary.chapters),
                    stat_row("Escenas", summary.scenes),
                    stat_row("Personajes", summary.characters),
                    stat_row("Relaciones", summary.relationships),
                    stat_row("Hilos/teorías", summary.theories),
                    stat_row("Eventos cronológicos", summary.timeline),
                    stat_row("Entradas de mundo", summary.world),
                    stat_row("Mapas", summary.maps),
                    stat_row("Sistemas de magia", summary.magic),
                    stat_row("Referencias", summary.references),
                    stat_row("Documentos / notas", summary.notes),
                    stat_row("Notas de revisión", summary.review),
                    stat_row("Medios", summary.media),
                ];
            }
            _ => {}
        }
        self.array(section)
            .iter()
            .map(|row| {
                let label = row
                    .get("title")
                    .or_else(|| row.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("Sin título")
                    .to_owned();
                let detail = row
                    .get("status")
                    .and_then(Value::as_str)
                    .or_else(|| row.get("summary").and_then(Value::as_str))
                    .or_else(|| row.get("type").and_then(Value::as_str))
                    .unwrap_or("")
                    .to_owned();
                let path = row.get("path").and_then(Value::as_str).map(PathBuf::from);
                WorldRow { label, detail, path }
            })
            .collect()
    }

    pub fn record_fields(&self, section: WorldSection, index: usize) -> Vec<WorldRow> {
        if section.is_record_list() {
            return self.array(section).get(index).map(|value| object_rows(Some(value))).unwrap_or_default();
        }
        match section {
            WorldSection::Project => object_rows(self.root.get("profile")),
            WorldSection::Layout => object_rows(self.root.get("layout")),
            WorldSection::Settings => object_rows(self.root.get("settings")),
            _ => Vec::new(),
        }
    }

    pub fn set_field(&mut self, section: WorldSection, index: usize, key: &str, raw: &str) -> Result<bool> {
        let target = if section.is_record_list() {
            self.array_mut(section).get_mut(index)
        } else {
            match section {
                WorldSection::Project => self.root.get_mut("profile"),
                WorldSection::Layout => self.root.get_mut("layout"),
                WorldSection::Settings => self.root.get_mut("settings"),
                _ => None,
            }
        };
        let Some(object) = target.and_then(Value::as_object_mut) else { return Ok(false); };
        let existing = object.get(key).cloned().unwrap_or(Value::String(String::new()));
        let value = parse_like(&existing, raw);
        object.insert(key.to_owned(), value);
        if section == WorldSection::Scenes {
            self.sync_scene_document(index)?;
        }
        self.touch();
        self.save()?;
        Ok(true)
    }

    pub fn create_chapter(&mut self, title: &str) -> Result<PathBuf> {
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
                    "status": "draft", "summary": "", "words": 0, "updated": unix_seconds(), "order": order
                })
            },
            WorldSection::Characters => json!({
                "id": generate_id(), "name": title, "image": "", "aliases": [], "category": "", "status": "",
                "role": "", "occupation": "", "origin": "", "affiliation": "", "summary": "",
                "background": "", "physical": "", "traits": [], "evidence": [], "chapters": [], "color": "", "x": 0, "y": 0
            }),
            WorldSection::Relationships => json!({
                "id": generate_id(), "title": title, "from": "", "to": "", "type": "",
                "strength": 0, "detail": "", "certainty": "", "visible": true
            }),
            WorldSection::Theories => json!({
                "id": generate_id(), "title": title, "status": "open", "confidence": 0, "thesis": "",
                "evidence": [], "counterargument": "", "characters": [], "tags": []
            }),
            WorldSection::Timeline => json!({
                "id": generate_id(), "title": title, "chapter": null, "position": "", "summary": "",
                "characters": [], "intensity": 0, "order": 0
            }),
            WorldSection::World => json!({
                "id": generate_id(), "name": title, "type": "Otro", "aliases": [], "summary": "",
                "geography": "", "government": "", "peoples": "", "culture": "", "economy": "",
                "currency": "", "languages": [], "religions": [], "military": "", "history": "",
                "relations": [], "locations": [], "conflicts": [], "notes": "", "tags": [], "attachments": []
            }),
            WorldSection::Maps => json!({
                "id": generate_id(), "name": title, "description": "", "seed": unix_seconds(), "style": "Parchment",
                "continents": 1, "islands": 0, "roughness": 0.5, "background": "", "markers": [], "layers": [],
                "viewport": {"zoom": 1.0, "x": 0.0, "y": 0.0}
            }),
            WorldSection::Magic => json!({
                "id": generate_id(), "name": title, "category": "", "status": "development", "source": "",
                "principle": "", "access": "", "cost": "", "limits": "", "manifestations": "",
                "materials": [], "institutions": [], "users": [], "risks": "", "history": "",
                "notes": "", "evidence": [], "tags": [], "attachments": []
            }),
            WorldSection::References => json!({
                "id": generate_id(), "title": title, "type": "reference", "target": "", "source_name": "",
                "imported_at": unix_seconds(), "summary": "", "tags": [], "mentions": []
            }),
            WorldSection::Notes => json!({
                "id": generate_id(), "title": title, "body": "", "category": "documentation", "tags": [],
                "references": [], "attachments": [], "updated": unix_seconds()
            }),
            WorldSection::Review => json!({
                "id": generate_id(), "title": title, "type": "note", "status": "open", "chapter": null,
                "message": "", "context": "", "replacement": "", "tags": []
            }),
            WorldSection::Media => json!({
                "id": generate_id(), "title": title, "type": "local", "path": "", "url": "", "provider": "",
                "mime": "", "notes": "", "package_path": ""
            }),
            _ => return Ok(()),
        };
        self.array_mut(section).push(record);
        self.touch();
        self.save()
    }

    pub fn delete_record(&mut self, section: WorldSection, index: usize) -> Result<bool> {
        if !section.is_record_list() {
            return Ok(false);
        }
        let rows = self.array_mut(section);
        if index >= rows.len() {
            return Ok(false);
        }
        let removed = rows.remove(index);
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
    }

    pub fn move_record(&mut self, section: WorldSection, index: usize, delta: isize) -> Result<usize> {
        if !section.is_record_list() {
            return Ok(index);
        }
        let rows = self.array_mut(section);
        if rows.is_empty() || index >= rows.len() {
            return Ok(index);
        }
        let target = (index as isize + delta).clamp(0, rows.len() as isize - 1) as usize;
        if target != index {
            let value = rows.remove(index);
            rows.insert(target, value);
            for (order, row) in rows.iter_mut().enumerate() {
                if let Some(object) = row.as_object_mut() {
                    object.insert("order".into(), json!((order as i64 + 1) * 10));
                }
            }
            let scene_count = if section == WorldSection::Scenes { rows.len() } else { 0 };
            let _ = rows;
            for scene_index in 0..scene_count {
                self.sync_scene_document(scene_index)?;
            }
            self.touch();
            self.save()?;
        }
        Ok(target)
    }

    pub fn add_media(&mut self, title: &str, path: &Path) -> Result<()> {
        let kind = path.extension().and_then(|v| v.to_str()).unwrap_or("").to_ascii_lowercase();
        let media_type = match kind.as_str() {
            "mp3" | "wav" | "flac" | "ogg" | "m4a" => "audio",
            "mp4" | "webm" | "mov" | "avi" | "mkv" | "vob" => "video",
            _ => "file",
        };
        self.array_mut(WorldSection::Media).push(json!({
            "id": generate_id(), "title": title, "type": media_type,
            "path": path.to_string_lossy(), "url": "", "provider": "local", "mime": "", "notes": "", "package_path": ""
        }));
        self.touch();
        self.save()
    }

    fn sync_scene_document(&mut self, index: usize) -> Result<()> {
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

    fn sync_manuscript(&mut self, documents: &Path) -> Result<()> {
        let mut manuscript = Vec::new();
        let read_dir = fs::read_dir(documents)
            .with_context(|| format!("No se pudo leer {}", documents.display()))?;
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_file() || !document::is_native_path(&path) {
                continue;
            }
            let Some(metadata) = document::read_metadata(&path) else { continue; };
            if metadata.project.trim() != self.name || metadata.kind.eq_ignore_ascii_case("scene") {
                continue;
            }
            let words = document::read(&path)
                .map(|doc| doc.body.split_whitespace().count())
                .unwrap_or_else(|_| fs::read_to_string(&path).map(|text| text.split_whitespace().count()).unwrap_or(0));
            manuscript.push(json!({
                "id": metadata.id,
                "title": metadata.title,
                "path": path.to_string_lossy(),
                "kind": metadata.kind,
                "chapter": metadata.chapter,
                "order": metadata.order,
                "status": metadata.status,
                "words": words
            }));
        }
        manuscript.sort_by(|a, b| {
            let ao = a.get("order").and_then(Value::as_i64).unwrap_or(0);
            let bo = b.get("order").and_then(Value::as_i64).unwrap_or(0);
            let ac = a.get("chapter").and_then(Value::as_u64).unwrap_or(u64::MAX);
            let bc = b.get("chapter").and_then(Value::as_u64).unwrap_or(u64::MAX);
            ao.cmp(&bo).then(ac.cmp(&bc))
        });
        *self.array_mut(WorldSection::Manuscript) = manuscript;
        Ok(())
    }

    fn normalize(&mut self) {
        if !self.root.is_object() {
            self.root = empty_project(&self.name);
        }
        let object = self.root.as_object_mut().expect("normalized object");
        object.insert("format".into(), json!(PROJECT_FORMAT));
        object.entry("name").or_insert(json!(self.name));
        object.entry("created").or_insert(json!(unix_seconds()));
        object.entry("updated").or_insert(json!(unix_seconds()));
        object.entry("profile").or_insert(json!({
            "title": self.name, "subtitle": "", "author": "", "genre": "", "status": "draft",
            "synopsis": "", "location": "", "chapter_labels": [], "tags": [], "cover": "", "banner": "", "icon": ""
        }));
        object.entry("layout").or_insert(default_layout());
        object.entry("settings").or_insert(default_settings());
        object.entry("review_config").or_insert(json!({"repetitions": true, "filler": [], "language": "es-CL"}));
        object.entry("sections").or_insert_with(|| json!({}));
        let sections = object
            .get_mut("sections")
            .and_then(Value::as_object_mut)
            .expect("sections object");
        for section in WorldSection::ALL {
            if matches!(section, WorldSection::Project | WorldSection::Layout | WorldSection::Settings | WorldSection::Statistics) {
                continue;
            }
            sections.entry(section.key()).or_insert_with(|| json!([]));
        }
    }

    fn touch(&mut self) {
        if let Some(root) = self.root.as_object_mut() {
            root.insert("updated".into(), json!(unix_seconds()));
        }
    }

    fn array(&self, section: WorldSection) -> &[Value] {
        self.root
            .get("sections")
            .and_then(|value| value.get(section.key()))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn array_mut(&mut self, section: WorldSection) -> &mut Vec<Value> {
        self.root
            .as_object_mut()
            .unwrap()
            .get_mut("sections")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .entry(section.key())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .unwrap()
    }
}

fn unique_scene_path(documents: &Path, project: &str, title: &str) -> PathBuf {
    let base = format!("{} - {}", safe_component(project), safe_component(title));
    let mut path = documents.join(format!("{base}.txt"));
    for index in 2..10_000 {
        if !path.exists() { break; }
        path = documents.join(format!("{base} ({index}).txt"));
    }
    path
}

pub fn list_projects(documents: &Path) -> Vec<String> {
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

fn empty_project(name: &str) -> Value {
    json!({
        "format": PROJECT_FORMAT,
        "name": name,
        "created": unix_seconds(),
        "updated": unix_seconds(),
        "profile": {
            "title": name, "subtitle": "", "author": "", "genre": "", "status": "draft",
            "synopsis": "", "location": "", "chapter_labels": [], "tags": [], "cover": "", "banner": "", "icon": ""
        },
        "layout": default_layout(),
        "settings": default_settings(),
        "review_config": {"repetitions": true, "filler": [], "language": "es-CL"},
        "sections": {
            "manuscript": [], "scenes": [], "characters": [], "relationships": [], "theories": [],
            "timeline": [], "world": [], "maps": [], "magic": [], "references": [], "notes": [], "review": [], "media": []
        }
    })
}

fn default_layout() -> Value {
    json!({
        "preset": "Editorial", "paper_width_mm": 152.0, "paper_height_mm": 229.0,
        "margin_top_mm": 18.0, "margin_bottom_mm": 20.0, "margin_inner_mm": 20.0, "margin_outer_mm": 16.0,
        "font_family": "EB Garamond", "font_size_pt": 11.5, "line_height": 1.2, "indent_em": 1.2,
        "chapter_opening": "new-page", "scene_separator": "⁂", "header": "{title}", "footer": "{page}"
    })
}

fn default_settings() -> Value {
    json!({
        "theme": "dark", "theme_background": "#282828", "theme_foreground": "#dfe8ef", "theme_accent": "#e8cc83",
        "autosave": true, "spellcheck": true, "language": "es-CL",
        "focus_fullscreen": true, "backup_local": true, "backup_drive": false,
        "editor_font": "JetBrains Mono", "editor_font_size": 14,
        "map_style": "Parchment", "map_grid": true,
        "review_fillers": ["realmente", "básicamente", "literalmente", "simplemente", "de alguna manera", "en realidad", "de hecho"]
    })
}

fn object_rows(value: Option<&Value>) -> Vec<WorldRow> {
    value
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| WorldRow {
                    label: key.clone(),
                    detail: scalar_text(value),
                    path: None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_like(existing: &Value, raw: &str) -> Value {
    match existing {
        Value::Bool(_) => Value::Bool(matches!(raw.trim().to_ascii_lowercase().as_str(), "1" | "true" | "sí" | "si" | "yes" | "on")),
        Value::Number(number) if number.is_i64() => raw.trim().parse::<i64>().map(Value::from).unwrap_or_else(|_| existing.clone()),
        Value::Number(number) if number.is_u64() => raw.trim().parse::<u64>().map(Value::from).unwrap_or_else(|_| existing.clone()),
        Value::Number(_) => raw.trim().parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number).unwrap_or_else(|| existing.clone()),
        Value::Array(_) => Value::Array(raw.split(',').map(str::trim).filter(|v| !v.is_empty()).map(|v| Value::String(v.to_owned())).collect()),
        Value::Null if raw.trim().is_empty() => Value::Null,
        Value::Null => Value::String(raw.to_owned()),
        Value::Object(_) => existing.clone(),
        Value::String(_) => Value::String(raw.to_owned()),
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        Value::Array(values) => values.iter().map(scalar_text).collect::<Vec<_>>().join(", "),
        Value::Object(_) => "…".into(),
    }
}

fn stat_row(label: &str, value: usize) -> WorldRow {
    WorldRow {
        label: label.into(),
        detail: value.to_string(),
        path: None,
    }
}

fn safe_component(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let cleaned = cleaned.trim_matches(['_', '.']);
    if cleaned.is_empty() { "project".into() } else { cleaned.into() }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn generate_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("wbw-{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_inventory_is_complete() {
        assert_eq!(WorldSection::ALL.len(), 17);
        assert_eq!(WorldSection::Characters.label(), "Personajes");
        assert!(WorldSection::Maps.is_record_list());
        assert!(!WorldSection::Layout.is_record_list());
    }

    #[test]
    fn project_file_name_is_hidden_and_safe() {
        let path = project_path(Path::new("docs"), "Puerto Ámbar / Libro 1");
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with(".zen-project-"));
        assert!(name.ends_with(".json"));
        assert!(!name.contains('/'));
    }
}
