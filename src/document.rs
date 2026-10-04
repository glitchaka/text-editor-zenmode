use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};
use zip::{ZipArchive, ZipWriter, write::FileOptions};

use crate::page::{PageOrientation, PageProfile, PaperSize};

pub const EXTENSION: &str = "hsst";
pub const FORMAT_VERSION: u32 = 2;

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
    pub page: PageProfile,
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
            page: PageProfile::default(),
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
    write(path, &document)
}

pub fn read(path: &Path) -> Result<HsstDocument> {
    let bytes = fs::read(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
    if bytes.starts_with(b"PK\x03\x04") {
        return read_container(path);
    }
    let raw = String::from_utf8(bytes).with_context(|| {
        format!(
            "{} no es UTF-8 ni un contenedor HSST válido",
            path.display()
        )
    })?;
    Ok(parse(&raw, path))
}

pub fn write(path: &Path, document: &HsstDocument) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let (plain, formatting) = formatting_payload(&document.body);
    let manifest = manifest_json(&document.metadata);
    let temporary = path.with_extension("hsst.tmp");
    let backup = path.with_extension("hsst.bak");

    {
        let file = fs::File::create(&temporary)
            .with_context(|| format!("No se pudo crear {}", temporary.display()))?;
        let mut archive = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        archive.start_file("manifest.json", options)?;
        archive.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
        archive.start_file("content.txt", options)?;
        archive.write_all(plain.as_bytes())?;
        archive.start_file("formatting.json", options)?;
        archive.write_all(serde_json::to_string_pretty(&formatting)?.as_bytes())?;
        archive.start_file("history.jsonl", options)?;
        archive.write_all(b"")?;
        archive.finish()?;
    }

    if backup.exists() {
        let _ = fs::remove_file(&backup);
    }
    if path.exists() {
        fs::rename(path, &backup)
            .with_context(|| format!("No se pudo preparar reemplazo de {}", path.display()))?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(error).with_context(|| format!("No se pudo guardar {}", path.display()));
    }
    if backup.exists() {
        let _ = fs::remove_file(&backup);
    }
    Ok(())
}

fn read_container(path: &Path) -> Result<HsstDocument> {
    let file = fs::File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("{} no es un contenedor HSST válido", path.display()))?;

    let manifest: Value = {
        let mut raw = String::new();
        archive.by_name("manifest.json")?.read_to_string(&mut raw)?;
        serde_json::from_str(&raw)?
    };
    let content = {
        let mut raw = String::new();
        archive.by_name("content.txt")?.read_to_string(&mut raw)?;
        raw
    };
    let formatting: Value = match archive.by_name("formatting.json") {
        Ok(mut entry) => {
            let mut raw = String::new();
            entry.read_to_string(&mut raw)?;
            serde_json::from_str(&raw).unwrap_or_else(|_| json!({"version": 1, "runs": []}))
        }
        Err(_) => json!({"version": 1, "runs": []}),
    };

    Ok(HsstDocument {
        metadata: metadata_from_manifest(&manifest, path),
        body: body_from_parts(&content, &formatting),
    })
}

fn manifest_json(metadata: &DocumentMetadata) -> Value {
    let page = metadata.page;
    json!({
        "format": FORMAT_VERSION,
        "id": metadata.id,
        "title": metadata.title,
        "project": metadata.project,
        "type": metadata.kind,
        "chapter": metadata.chapter,
        "order": metadata.order,
        "language": metadata.language,
        "status": metadata.status,
        "page": {
            "paper": page.paper.name(),
            "orientation": page.orientation.name(),
            "margin_top_mm": page.margin_top_mm,
            "margin_right_mm": page.margin_right_mm,
            "margin_bottom_mm": page.margin_bottom_mm,
            "margin_left_mm": page.margin_left_mm
        }
    })
}

fn metadata_from_manifest(value: &Value, source: &Path) -> DocumentMetadata {
    let fallback = fallback_metadata(source);
    let page_value = value.get("page").unwrap_or(&Value::Null);
    let margin = |key: &str, fallback_value: u16| {
        page_value
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| (5..=60).contains(value))
            .unwrap_or(fallback_value)
    };
    let page = PageProfile {
        paper: page_value
            .get("paper")
            .and_then(Value::as_str)
            .and_then(PaperSize::parse)
            .unwrap_or(fallback.page.paper),
        orientation: page_value
            .get("orientation")
            .and_then(Value::as_str)
            .and_then(PageOrientation::parse)
            .unwrap_or(fallback.page.orientation),
        margin_top_mm: margin("margin_top_mm", fallback.page.margin_top_mm),
        margin_right_mm: margin("margin_right_mm", fallback.page.margin_right_mm),
        margin_bottom_mm: margin("margin_bottom_mm", fallback.page.margin_bottom_mm),
        margin_left_mm: margin("margin_left_mm", fallback.page.margin_left_mm),
    };

    DocumentMetadata {
        format: value
            .get("format")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(FORMAT_VERSION),
        id: value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(&fallback.id)
            .to_owned(),
        title: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or(&fallback.title)
            .to_owned(),
        project: value
            .get("project")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        kind: value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("document")
            .to_owned(),
        chapter: value
            .get("chapter")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0),
        order: value.get("order").and_then(Value::as_i64).unwrap_or(0),
        language: value
            .get("language")
            .and_then(Value::as_str)
            .unwrap_or("es-CL")
            .to_owned(),
        status: value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("draft")
            .to_owned(),
        page,
    }
}

fn formatting_payload(body: &str) -> (String, Value) {
    let mut plain = String::new();
    let mut ranges = Vec::new();

    for run in crate::format::styled_runs(body) {
        let start = plain.len();
        plain.push_str(&run.text);
        let end = plain.len();
        if start == end || run.style == crate::format::TextStyle::default() {
            continue;
        }

        let mut object = Map::new();
        object.insert("start".into(), json!(start));
        object.insert("end".into(), json!(end));
        if run.style.bold {
            object.insert("bold".into(), json!(true));
        }
        if run.style.italic {
            object.insert("italic".into(), json!(true));
        }
        if run.style.underline {
            object.insert("underline".into(), json!(true));
        }
        if let Some(color) = run.style.foreground {
            object.insert("foreground".into(), json!(color.name()));
        }
        if let Some(color) = run.style.background {
            object.insert("background".into(), json!(color.name()));
        }
        ranges.push(Value::Object(object));
    }

    (
        plain,
        json!({
            "version": 1,
            "unit": "utf8-byte",
            "runs": ranges
        }),
    )
}

fn body_from_parts(content: &str, formatting: &Value) -> String {
    let mut ranges = formatting
        .get("runs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| {
            let start = value
                .get("start")?
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())?;
            let end = value
                .get("end")?
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())?;
            let style = crate::format::TextStyle {
                bold: value.get("bold").and_then(Value::as_bool).unwrap_or(false),
                italic: value
                    .get("italic")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                underline: value
                    .get("underline")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                foreground: value
                    .get("foreground")
                    .and_then(Value::as_str)
                    .and_then(crate::format::PaletteColor::parse),
                background: value
                    .get("background")
                    .and_then(Value::as_str)
                    .and_then(crate::format::PaletteColor::parse),
            };
            Some((start, end, style))
        })
        .collect::<Vec<_>>();
    ranges.sort_by_key(|(start, end, _)| (*start, *end));

    let mut output = String::with_capacity(content.len());
    let mut cursor = 0usize;
    for (start, end, style) in ranges {
        if start < cursor
            || start >= end
            || end > content.len()
            || !content.is_char_boundary(start)
            || !content.is_char_boundary(end)
        {
            continue;
        }
        output.push_str(&content[cursor..start]);
        output.push_str(&encode_styled_segment(&content[start..end], style));
        cursor = end;
    }
    output.push_str(&content[cursor..]);
    output
}

fn encode_styled_segment(segment: &str, style: crate::format::TextStyle) -> String {
    let mut output = segment.to_owned();
    if style.underline {
        output = format!("__{output}__");
    }
    if style.italic {
        output = format!("*{output}*");
    }
    if style.bold {
        output = format!("**{output}**");
    }
    if let Some(color) = style.background {
        output = format!("{{{{bg:{}}}}}{output}{{{{/bg}}}}", color.name());
    }
    if let Some(color) = style.foreground {
        output = format!("{{{{fg:{}}}}}{output}{{{{/fg}}}}", color.name());
    }
    output
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
    let page = metadata.page;

    format!(
        "+++\nformat = {}\nid = \"{}\"\ntitle = \"{}\"\nproject = \"{}\"\ntype = \"{}\"\nchapter = {}\norder = {}\nlanguage = \"{}\"\nstatus = \"{}\"\npaper = \"{}\"\norientation = \"{}\"\nmargin_top_mm = {}\nmargin_right_mm = {}\nmargin_bottom_mm = {}\nmargin_left_mm = {}\n+++\n\n{}",
        metadata.format.max(1),
        toml_escape(&metadata.id),
        toml_escape(&metadata.title),
        toml_escape(&metadata.project),
        toml_escape(&metadata.kind),
        chapter,
        metadata.order,
        toml_escape(&metadata.language),
        toml_escape(&metadata.status),
        page.paper.name(),
        page.orientation.name(),
        page.margin_top_mm,
        page.margin_right_mm,
        page.margin_bottom_mm,
        page.margin_left_mm,
        document.body
    )
}

pub fn set_page_profile(path: &Path, page: PageProfile) -> Result<()> {
    if !is_native_path(path) {
        return Ok(());
    }
    let mut document = read(path)?;
    if document.metadata.page == page {
        return Ok(());
    }
    document.metadata.page = page;
    write(path, &document).with_context(|| {
        format!(
            "No se pudo actualizar el perfil de página de {}",
            path.display()
        )
    })
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
    let margin = |key: &str, fallback_value: u16| {
        table
            .get(key)
            .and_then(toml::Value::as_integer)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| (5..=60).contains(value))
            .unwrap_or(fallback_value)
    };
    let page = PageProfile {
        paper: table
            .get("paper")
            .and_then(toml::Value::as_str)
            .and_then(PaperSize::parse)
            .unwrap_or(fallback.page.paper),
        orientation: table
            .get("orientation")
            .and_then(toml::Value::as_str)
            .and_then(PageOrientation::parse)
            .unwrap_or(fallback.page.orientation),
        margin_top_mm: margin("margin_top_mm", fallback.page.margin_top_mm),
        margin_right_mm: margin("margin_right_mm", fallback.page.margin_right_mm),
        margin_bottom_mm: margin("margin_bottom_mm", fallback.page.margin_bottom_mm),
        margin_left_mm: margin("margin_left_mm", fallback.page.margin_left_mm),
    };

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
        page,
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
                page: PageProfile::default(),
            },
            body: "Texto **en negrita**.\n".into(),
        };

        let encoded = serialize(&document);
        let decoded = parse(&encoded, Path::new("Capítulo 1.hsst"));
        assert_eq!(decoded.metadata, document.metadata);
        assert_eq!(decoded.body, document.body);
        assert_eq!(decoded.metadata.page.paper, PaperSize::Letter);
        assert_eq!(decoded.metadata.page.margin_left_mm, 25);
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
