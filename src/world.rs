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
    Review,
    Layout,
    Media,
    Settings,
    Statistics,
}

impl WorldSection {
    pub const ALL: [Self; 16] = [
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
            root,
        };
        workspace.normalize();
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
        if let Some(media) = root.get_mut("sections").and_then(|v| v.get_mut("media")).and_then(Value::as_array_mut) {
            let media_root = documents.join(".zen-media").join(safe_component(&name));
            fs::create_dir_all(&media_root)?;
            for item in media.iter_mut() {
                let Some(source) = item.get("package_path").and_then(Value::as_str) else { continue; };
                let Ok(mut zipped) = archive.by_name(source) else { continue; };
                let file_name = Path::new(source)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .map(safe_component)
                    .unwrap_or_else(|| format!("media-{}", generate_id()));
                let target = media_root.join(file_name);
                let mut bytes = Vec::new();
                zipped.read_to_end(&mut bytes)?;
                fs::write(&target, bytes)?;
                if let Some(object) = item.as_object_mut() {
                    object.insert("path".into(), json!(target.to_string_lossy()));
                }
            }
        }
        let mut workspace = Self {
            name: name.clone(),
            path: project_path(documents, &name),
            root,
        };
        workspace.normalize();
        workspace.sync_manuscript(documents)?;
        workspace.save()?;
        Ok(workspace)
    }

    pub fn export_package(&self, documents: &Path, destination: &Path) -> Result<PathBuf> {
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
        zip.start_file(PACKAGE_MANIFEST, options)?;
        zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
        zip.start_file(PACKAGE_PROJECT, options)?;
        zip.write_all(serde_json::to_string_pretty(&self.root)?.as_bytes())?;

        let mut included = std::collections::HashSet::new();
        for row in self.array(WorldSection::Manuscript) {
            let Some(path) = row.get("path").and_then(Value::as_str).map(PathBuf::from) else { continue; };
            if !path.is_file() { continue; }
            let name = path.file_name().and_then(|v| v.to_str()).map(safe_component).unwrap_or_else(|| generate_id());
            let package_path = format!("manuscript/{name}");
            if included.insert(package_path.clone()) {
                zip.start_file(package_path, options)?;
                zip.write_all(&fs::read(&path)?)?;
            }
            let sidecar = path.with_file_name(format!("{}.zen.json", path.file_name().and_then(|v| v.to_str()).unwrap_or("document.txt")));
            if sidecar.is_file() {
                let side_name = sidecar.file_name().and_then(|v| v.to_str()).map(safe_component).unwrap_or_else(|| format!("{}.zen.json", generate_id()));
                let package_path = format!("manuscript/{side_name}");
                if included.insert(package_path.clone()) {
                    zip.start_file(package_path, options)?;
                    zip.write_all(&fs::read(sidecar)?)?;
                }
            }
        }
        for row in self.array(WorldSection::Media) {
            let Some(path) = row.get("path").and_then(Value::as_str).map(PathBuf::from) else { continue; };
            if !path.is_file() { continue; }
            let name = path.file_name().and_then(|v| v.to_str()).map(safe_component).unwrap_or_else(|| generate_id());
            let package_path = format!("media/{name}");
            if included.insert(package_path.clone()) {
                zip.start_file(package_path, options)?;
                zip.write_all(&fs::read(path)?)?;
            }
        }
        let _ = documents;
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
        summary.review = self.array(WorldSection::Review).len();
        summary.media = self.array(WorldSection::Media).len();
        summary.words = self
            .array(WorldSection::Manuscript)
            .iter()
            .filter_map(|row| row.get("words").and_then(Value::as_u64))
            .map(|value| value as usize)
            .sum();
        summary
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

    pub fn add_record(&mut self, section: WorldSection, title: &str) -> Result<()> {
        if !section.is_record_list() {
            return Ok(());
        }
        let record = match section {
            WorldSection::Scenes => json!({
                "id": generate_id(), "title": title, "chapter": null, "pov": "", "location": "",
                "timeline": "", "narrative_layer": "", "status": "draft", "summary": "", "words": 0, "order": 0
            }),
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
        rows.remove(index);
        self.touch();
        self.save()?;
        Ok(true)
    }

    pub fn rename_record(&mut self, section: WorldSection, index: usize, title: &str) -> Result<bool> {
        if !section.is_record_list() || title.trim().is_empty() {
            return Ok(false);
        }
        let Some(record) = self.array_mut(section).get_mut(index).and_then(Value::as_object_mut) else {
            return Ok(false);
        };
        let key = if record.contains_key("name") { "name" } else { "title" };
        record.insert(key.into(), json!(title.trim()));
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
                    object.insert("order".into(), json!(order as i64));
                }
            }
            self.touch();
            self.save()?;
        }
        Ok(target)
    }

    pub fn set_profile_field(&mut self, key: &str, value: &str) -> Result<()> {
        if let Some(profile) = self.root.get_mut("profile").and_then(Value::as_object_mut) {
            profile.insert(key.to_owned(), json!(value));
        }
        self.touch();
        self.save()
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
            if metadata.project.trim() != self.name {
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
            "timeline": [], "world": [], "maps": [], "magic": [], "references": [], "review": [], "media": []
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
        "theme": "dark", "autosave": true, "spellcheck": true, "language": "es-CL",
        "focus_fullscreen": true, "backup_local": true, "backup_drive": false,
        "editor_font": "JetBrains Mono", "editor_font_size": 14,
        "map_style": "Parchment", "map_grid": true
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
        assert_eq!(WorldSection::ALL.len(), 16);
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
