use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};

pub const EXTENSION: &str = "hsst";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocumentMetadata {
    pub format: u32,
    pub id: String,
    pub title: String,
    pub project: String,
    pub kind: String,
    pub chapter: Option<u32>,
    pub order: i64,
    pub language: String,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct HsstDocument {
    pub metadata: DocumentMetadata,
    pub body: String,
}

impl DocumentMetadata {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            format: FORMAT_VERSION,
            id: generate_id(),
            title: title.into(),
            project: String::new(),
            kind: "document".into(),
            chapter: None,
            order: 0,
            language: "es-CL".into(),
            status: "draft".into(),
        }
    }

    pub fn chapter_label(&self) -> Option<String> {
        self.chapter.map(|number| format!("cap. {number}"))
    }
}

pub fn is_native_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION))
}

pub fn is_supported_text_path(path: &Path) -> bool {
    if path.is_dir() {
        return true;
    }

    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("hsst" | "txt" | "text" | "md" | "markdown" | "rst") | None
    )
}

pub fn native_path_for_name(directory: &Path, requested_name: &str) -> PathBuf {
    let trimmed = requested_name.trim();
    let mut path = directory.join(trimmed);
    if !is_native_path(&path) {
        path.set_extension(EXTENSION);
    }
    path
}

pub fn create_native(path: &Path, title: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let document = HsstDocument {
        metadata: DocumentMetadata::new(title),
        body: String::new(),
    };
    fs::write(path, serialize(&document))
        .with_context(|| format!("No se pudo crear {}", path.display()))
}

pub fn read(path: &Path) -> Result<HsstDocument> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
    Ok(parse(&raw, path))
}

pub fn read_metadata(path: &Path) -> Option<DocumentMetadata> {
    if !is_native_path(path) {
        return None;
    }
    read(path).ok().map(|document| document.metadata)
}

pub fn parse(raw: &str, source: &Path) -> HsstDocument {
    let Some((frontmatter, body)) = split_frontmatter(raw) else {
        return HsstDocument {
            metadata: fallback_metadata(source),
            body: raw.to_owned(),
        };
    };

    let value = frontmatter.parse::<toml::Value>().ok();
    let metadata = value
        .as_ref()
        .and_then(toml::Value::as_table)
        .map(|table| metadata_from_table(table, source))
        .unwrap_or_else(|| fallback_metadata(source));

    HsstDocument {
        metadata,
        body: body.to_owned(),
    }
}

pub fn serialize(document: &HsstDocument) -> String {
    let metadata = &document.metadata;
    let chapter = metadata
        .chapter
        .map(|value| value.to_string())
        .unwrap_or_else(|| "0".into());

    format!(
        "+++\nformat = {}\nid = \"{}\"\ntitle = \"{}\"\nproject = \"{}\"\ntype = \"{}\"\nchapter = {}\norder = {}\nlanguage = \"{}\"\nstatus = \"{}\"\n+++\n\n{}",
        metadata.format.max(1),
        toml_escape(&metadata.id),
        toml_escape(&metadata.title),
        toml_escape(&metadata.project),
        toml_escape(&metadata.kind),
        chapter,
        metadata.order,
        toml_escape(&metadata.language),
        toml_escape(&metadata.status),
        document.body
    )
}

pub fn project_documents(root: &Path, project: &str) -> Result<Vec<(PathBuf, DocumentMetadata)>> {
    let mut documents = Vec::new();
    if project.trim().is_empty() {
        return Ok(documents);
    }

    for entry in
        fs::read_dir(root).with_context(|| format!("No se pudo leer {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || !is_native_path(&path) {
            continue;
        }
        let Some(metadata) = read_metadata(&path) else {
            continue;
        };
        if metadata.project.eq_ignore_ascii_case(project) {
            documents.push((path, metadata));
        }
    }

    documents.sort_by(|(path_a, a), (path_b, b)| {
        chapter_sort_key(a)
            .cmp(&chapter_sort_key(b))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            .then_with(|| path_a.cmp(path_b))
    });
    Ok(documents)
}

pub fn project_chapters(root: &Path, project: &str) -> Result<Vec<(PathBuf, DocumentMetadata)>> {
    let mut chapters = project_documents(root, project)?;
    chapters.retain(|(_, metadata)| {
        metadata.kind.eq_ignore_ascii_case("chapter") && metadata.chapter.is_some()
    });
    Ok(chapters)
}

pub fn project_counts(root: &Path) -> Vec<(String, usize)> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(metadata) = read_metadata(&path) else {
            continue;
        };
        let project = metadata.project.trim();
        if project.is_empty() {
            continue;
        }
        *counts.entry(project.to_owned()).or_default() += 1;
    }

    counts.into_iter().collect()
}

pub fn body_without_markup(body: &str) -> String {
    let mut output = String::with_capacity(body.len());

    for (line_index, line) in body.lines().enumerate() {
        if line_index > 0 {
            output.push('\n');
        }

        let line = line.strip_prefix("# ").unwrap_or(line);
        let plain = crate::format::strip_markup(line).replace("~~", "");
        output.push_str(&plain);
    }

    if body.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn split_frontmatter(raw: &str) -> Option<(&str, &str)> {
    let rest = raw
        .strip_prefix("+++\n")
        .or_else(|| raw.strip_prefix("+++\r\n"))?;
    let marker_lf = "\n+++\n";
    let marker_crlf = "\r\n+++\r\n";

    if let Some(index) = rest.find(marker_lf) {
        let frontmatter = &rest[..index];
        let body = &rest[index + marker_lf.len()..];
        let body = body.strip_prefix('\n').unwrap_or(body);
        return Some((frontmatter, body));
    }
    if let Some(index) = rest.find(marker_crlf) {
        let frontmatter = &rest[..index];
        let body = &rest[index + marker_crlf.len()..];
        let body = body.strip_prefix("\r\n").unwrap_or(body);
        return Some((frontmatter, body));
    }
    None
}

fn metadata_from_table(table: &toml::value::Table, source: &Path) -> DocumentMetadata {
    let fallback = fallback_metadata(source);
    let chapter = table
        .get("chapter")
        .and_then(toml::Value::as_integer)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0);

    DocumentMetadata {
        format: table
            .get("format")
            .and_then(toml::Value::as_integer)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(FORMAT_VERSION),
        id: table
            .get("id")
            .and_then(toml::Value::as_str)
            .unwrap_or(&fallback.id)
            .to_owned(),
        title: table
            .get("title")
            .and_then(toml::Value::as_str)
            .unwrap_or(&fallback.title)
            .to_owned(),
        project: table
            .get("project")
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_owned(),
        kind: table
            .get("type")
            .and_then(toml::Value::as_str)
            .unwrap_or("document")
            .to_owned(),
        chapter,
        order: table
            .get("order")
            .and_then(toml::Value::as_integer)
            .unwrap_or(0),
        language: table
            .get("language")
            .and_then(toml::Value::as_str)
            .unwrap_or("es-CL")
            .to_owned(),
        status: table
            .get("status")
            .and_then(toml::Value::as_str)
            .unwrap_or("draft")
            .to_owned(),
    }
}

fn fallback_metadata(source: &Path) -> DocumentMetadata {
    let title = source
        .file_stem()
        .or_else(|| source.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("Documento")
        .to_owned();
    DocumentMetadata::new(title)
}

fn chapter_sort_key(metadata: &DocumentMetadata) -> (u8, i64, u32) {
    match metadata.chapter {
        Some(chapter) => (0, metadata.order, chapter),
        None => (1, metadata.order, u32::MAX),
    }
}

fn generate_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{nanos:032x}-{:08x}", std::process::id())
}

fn toml_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_document_roundtrips_metadata_and_body() {
        let document = HsstDocument {
            metadata: DocumentMetadata {
                format: 1,
                id: "abc".into(),
                title: "Capítulo 1".into(),
                project: "Puerto Ámbar".into(),
                kind: "chapter".into(),
                chapter: Some(1),
                order: 10,
                language: "es-CL".into(),
                status: "draft".into(),
            },
            body: "Texto **en negrita**.\n".into(),
        };

        let encoded = serialize(&document);
        let decoded = parse(&encoded, Path::new("Capítulo 1.hsst"));
        assert_eq!(decoded.metadata, document.metadata);
        assert_eq!(decoded.body, document.body);
    }

    #[test]
    fn new_names_default_to_hsst() {
        let path = native_path_for_name(Path::new("docs"), "Capítulo 1");
        assert_eq!(path, Path::new("docs").join("Capítulo 1.hsst"));
    }

    #[test]
    fn plain_body_removes_native_rich_markup() {
        let body = "# Título\nUno **dos** *tres* ==cuatro== __cinco__ {{fg:red}}seis{{/fg}} {{bg:blue}}siete{{/bg}}.\n";
        assert_eq!(
            body_without_markup(body),
            "Título\nUno dos tres cuatro cinco seis siete.\n"
        );
    }
}
