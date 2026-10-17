use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use crate::document;

const PROJECT_FORMAT: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldSection {
    Manuscript,
    Scenes,
    Characters,
    Relationships,
    Timeline,
    World,
    Magic,
    References,
    Notes,
    Statistics,
}

impl WorldSection {
    pub const ALL: [Self; 10] = [
        Self::Manuscript,
        Self::Scenes,
        Self::Characters,
        Self::Relationships,
        Self::Timeline,
        Self::World,
        Self::Magic,
        Self::References,
        Self::Notes,
        Self::Statistics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Manuscript => "Manuscrito",
            Self::Scenes => "Escenas",
            Self::Characters => "Personajes",
            Self::Relationships => "Relaciones",
            Self::Timeline => "Cronología",
            Self::World => "Mundo / Atlas",
            Self::Magic => "Magia",
            Self::References => "Referencias",
            Self::Notes => "Documentación / notas",
            Self::Statistics => "Estadísticas",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Manuscript => "manuscript",
            Self::Scenes => "scenes",
            Self::Characters => "characters",
            Self::Relationships => "relationships",
            Self::Timeline => "timeline",
            Self::World => "world",
            Self::Magic => "magic",
            Self::References => "references",
            Self::Notes => "notes",
            Self::Statistics => "statistics",
        }
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
    pub timeline: usize,
    pub world: usize,
    pub magic: usize,
    pub references: usize,
    pub notes: usize,
    pub words: usize,
}

impl WorldSummary {
    pub fn count(&self, section: WorldSection) -> usize {
        match section {
            WorldSection::Manuscript => self.chapters,
            WorldSection::Scenes => self.scenes,
            WorldSection::Characters => self.characters,
            WorldSection::Relationships => self.relationships,
            WorldSection::Timeline => self.timeline,
            WorldSection::World => self.world,
            WorldSection::Magic => self.magic,
            WorldSection::References => self.references,
            WorldSection::Notes => self.notes,
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
        let mut workspace = Self { name: project.to_owned(), path, root };
        workspace.normalize();
        workspace.sync_manuscript(documents)?;
        workspace.save()?;
        Ok(workspace)
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
        summary.timeline = self.array(WorldSection::Timeline).len();
        summary.world = self.array(WorldSection::World).len();
        summary.magic = self.array(WorldSection::Magic).len();
        summary.references = self.array(WorldSection::References).len();
        summary.notes = self.array(WorldSection::Notes).len();
        summary.words = self.array(WorldSection::Manuscript)
            .iter()
            .filter_map(|row| row.get("words").and_then(Value::as_u64))
            .map(|value| value as usize)
            .sum();
        summary
    }

    pub fn rows(&self, section: WorldSection) -> Vec<WorldRow> {
        if section == WorldSection::Statistics {
            let summary = self.summary();
            return vec![
                WorldRow { label: "Palabras".into(), detail: summary.words.to_string(), path: None },
                WorldRow { label: "Capítulos/documentos".into(), detail: summary.chapters.to_string(), path: None },
                WorldRow { label: "Escenas".into(), detail: summary.scenes.to_string(), path: None },
                WorldRow { label: "Personajes".into(), detail: summary.characters.to_string(), path: None },
                WorldRow { label: "Entradas de mundo".into(), detail: summary.world.to_string(), path: None },
            ];
        }
        self.array(section)
            .iter()
            .map(|row| {
                let label = row.get("title")
                    .or_else(|| row.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("Sin título")
                    .to_owned();
                let detail = row.get("status").and_then(Value::as_str)
                    .or_else(|| row.get("summary").and_then(Value::as_str))
                    .unwrap_or("")
                    .to_owned();
                let path = row.get("path").and_then(Value::as_str).map(PathBuf::from);
                WorldRow { label, detail, path }
            })
            .collect()
    }

    pub fn add_record(&mut self, section: WorldSection, title: &str) -> Result<()> {
        if matches!(section, WorldSection::Manuscript | WorldSection::Statistics) {
            return Ok(());
        }
        let record = match section {
            WorldSection::Scenes => json!({
                "id": generate_id(), "title": title, "chapter": null, "pov": "", "location": "",
                "timeline": "", "narrative_layer": "", "status": "draft", "order": 0
            }),
            WorldSection::Characters => json!({
                "id": generate_id(), "name": title, "aliases": [], "category": "", "status": "",
                "role": "", "occupation": "", "origin": "", "affiliation": "", "summary": "",
                "background": "", "physical": "", "traits": [], "evidence": [], "chapters": []
            }),
            WorldSection::Relationships => json!({
                "id": generate_id(), "title": title, "from": "", "to": "", "type": "",
                "strength": 0, "detail": "", "certainty": ""
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
            WorldSection::Magic => json!({
                "id": generate_id(), "name": title, "category": "", "status": "development", "source": "",
                "principle": "", "access": "", "cost": "", "limits": "", "manifestations": "",
                "materials": [], "institutions": [], "users": [], "risks": "", "history": "",
                "notes": "", "evidence": [], "tags": [], "attachments": []
            }),
            WorldSection::References => json!({
                "id": generate_id(), "title": title, "type": "note", "target": "", "summary": "", "tags": []
            }),
            WorldSection::Notes => json!({
                "id": generate_id(), "title": title, "body": "", "tags": [], "references": []
            }),
            WorldSection::Manuscript | WorldSection::Statistics => return Ok(()),
        };
        self.array_mut(section).push(record);
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
        object.entry("format").or_insert(json!(PROJECT_FORMAT));
        object.entry("name").or_insert(json!(self.name));
        object.entry("profile").or_insert(json!({
            "title": self.name, "subtitle": "", "author": "", "genre": "", "status": "draft",
            "synopsis": "", "location": "", "chapter_labels": [], "tags": []
        }));
        object.entry("sections").or_insert_with(|| json!({}));
        let sections = object.get_mut("sections").and_then(Value::as_object_mut).expect("sections object");
        for section in WorldSection::ALL {
            if section != WorldSection::Statistics {
                sections.entry(section.key()).or_insert_with(|| json!([]));
            }
        }
    }

    fn array(&self, section: WorldSection) -> &[Value] {
        self.root.get("sections")
            .and_then(|value| value.get(section.key()))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn array_mut(&mut self, section: WorldSection) -> &mut Vec<Value> {
        self.root.as_object_mut().unwrap()
            .get_mut("sections").unwrap().as_object_mut().unwrap()
            .entry(section.key()).or_insert_with(|| json!([]))
            .as_array_mut().unwrap()
    }
}

pub fn project_path(documents: &Path, project: &str) -> PathBuf {
    let safe = project.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') { ch } else { '_' })
        .collect::<String>();
    documents.join(format!(".zen-project-{}.json", safe.trim_matches('_')))
}

fn empty_project(name: &str) -> Value {
    json!({
        "format": PROJECT_FORMAT,
        "name": name,
        "profile": {
            "title": name, "subtitle": "", "author": "", "genre": "", "status": "draft",
            "synopsis": "", "location": "", "chapter_labels": [], "tags": []
        },
        "sections": {
            "manuscript": [], "scenes": [], "characters": [], "relationships": [], "timeline": [],
            "world": [], "magic": [], "references": [], "notes": []
        }
    })
}

fn generate_id() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("wbw-{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_inventory_is_stable() {
        assert_eq!(WorldSection::ALL.len(), 10);
        assert_eq!(WorldSection::Characters.label(), "Personajes");
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
