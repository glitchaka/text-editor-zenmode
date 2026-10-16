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
    /// Texto plano que Helix edita. Nunca contiene marcas de formato HSST.
    pub body: String,
    /// Formato paralelo del contenido, almacenado en formatting.json.
    pub formatting: Value,
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
        formatting: empty_formatting(),
    };
    write(path, &document)
}


pub fn is_importable_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("txt" | "text" | "md" | "markdown" | "docx")
    )
}

pub fn import_to_native(source: &Path, directory: &Path) -> Result<PathBuf> {
    if !source.is_file() || !is_importable_path(source) {
        anyhow::bail!("Formato no importable: {}", source.display());
    }
    fs::create_dir_all(directory)?;
    let title = source
        .file_stem()
        .or_else(|| source.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("Documento")
        .to_owned();
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let (body, formatting) = if extension == "docx" {
        import_docx_body(source)?
    } else {
        let raw = fs::read_to_string(source)
            .with_context(|| format!("No se pudo importar {} como UTF-8", source.display()))?;
        if matches!(extension.as_str(), "md" | "markdown") {
            formatting_payload(&raw)
        } else {
            (raw, empty_formatting())
        }
    };

    let safe_title = safe_import_name(&title);
    let mut target = native_path_for_name(directory, &safe_title);
    if target.exists() {
        for index in 2..10_000 {
            let candidate = native_path_for_name(directory, &format!("{safe_title} ({index})"));
            if !candidate.exists() {
                target = candidate;
                break;
            }
        }
    }

    let document = HsstDocument {
        metadata: DocumentMetadata::new(title),
        body,
        formatting,
    };
    write(&target, &document)?;
    Ok(target)
}

fn safe_import_name(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => ch,
        })
        .collect::<String>();
    let cleaned = cleaned.trim().trim_end_matches('.');
    if cleaned.is_empty() {
        "Documento".into()
    } else {
        cleaned.into()
    }
}

fn import_docx_body(path: &Path) -> Result<(String, Value)> {
    let file = fs::File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("{} no es un DOCX válido", path.display()))?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .context("El DOCX no contiene word/document.xml")?
        .read_to_string(&mut xml)?;

    let mut rich = String::new();
    let mut cursor = 0usize;
    let mut plain_offset = 0usize;
    let mut paragraph_alignments = Vec::<(usize, usize, crate::format::ParagraphAlignment)>::new();
    while let Some(relative) = xml[cursor..].find("<w:p") {
        let start = cursor + relative;
        let Some(open_end_rel) = xml[start..].find('>') else {
            break;
        };
        let open_end = start + open_end_rel + 1;
        let Some(end_rel) = xml[open_end..].find("</w:p>") else {
            break;
        };
        let end = open_end + end_rel;
        let paragraph = &xml[start..end];
        if !rich.is_empty() {
            rich.push('\n');
            plain_offset += 1;
        }
        let paragraph_plain_start = plain_offset;
        let mut paragraph_plain_len = 0usize;
        let paragraph_alignment = xml_attr_after(paragraph, "<w:jc", "w:val")
            .as_deref()
            .and_then(crate::format::ParagraphAlignment::parse)
            .unwrap_or_default();

        let paragraph_style =
            xml_attr_after(paragraph, "<w:pStyle", "w:val").unwrap_or_default();
        if paragraph_style.eq_ignore_ascii_case("Heading1")
            || paragraph_style.eq_ignore_ascii_case("Title")
        {
            rich.push_str("# ");
            paragraph_plain_len += 2;
        } else if paragraph_style.eq_ignore_ascii_case("Heading2") {
            rich.push_str("## ");
            paragraph_plain_len += 3;
        } else if paragraph_style.eq_ignore_ascii_case("Heading3") {
            rich.push_str("### ");
            paragraph_plain_len += 4;
        }

        let mut run_cursor = 0usize;
        while let Some(run_rel) = paragraph[run_cursor..].find("<w:r") {
            let run_start = run_cursor + run_rel;
            let Some(run_open_rel) = paragraph[run_start..].find('>') else {
                break;
            };
            let run_open = run_start + run_open_rel + 1;
            let Some(run_end_rel) = paragraph[run_open..].find("</w:r>") else {
                break;
            };
            let run_end = run_open + run_end_rel;
            let run = &paragraph[run_start..run_end];
            let text = docx_run_text(run);
            if !text.is_empty() {
                let foreground = xml_attr_after(run, "<w:color", "w:val")
                    .as_deref()
                    .and_then(docx_foreground_color);
                let background = xml_attr_after(run, "<w:highlight", "w:val")
                    .as_deref()
                    .and_then(docx_highlight_color);
                let style = crate::format::TextStyle {
                    bold: has_enabled_word_tag(run, "w:b"),
                    italic: has_enabled_word_tag(run, "w:i"),
                    underline: has_enabled_word_tag(run, "w:u"),
                    strikethrough: has_enabled_word_tag(run, "w:strike"),
                    foreground,
                    background,
                };
                rich.push_str(&encode_styled_segment(&text, style));
                paragraph_plain_len += text.len();
            }
            run_cursor = run_end + "</w:r>".len();
        }
        let paragraph_plain_end = paragraph_plain_start + paragraph_plain_len;
        if paragraph_alignment != crate::format::ParagraphAlignment::Left {
            paragraph_alignments.push((paragraph_plain_start, paragraph_plain_end, paragraph_alignment));
        }
        plain_offset = paragraph_plain_end;
        cursor = end + "</w:p>".len();
    }

    let (body, formatting) = formatting_payload(&rich);
    let runs = decode_formatting_runs(&formatting);
    Ok((body, encode_formatting_parts(&runs, &paragraph_alignments)))
}

fn xml_attr_after(segment: &str, tag: &str, attr: &str) -> Option<String> {
    let start = segment.find(tag)?;
    let tail = &segment[start..];
    let tag_end = tail.find('>')?;
    let opening = &tail[..tag_end];
    let key = format!("{attr}=\"");
    let value_start = opening.find(&key)? + key.len();
    let value_end = opening[value_start..].find('"')? + value_start;
    Some(xml_unescape(&opening[value_start..value_end]))
}

fn has_enabled_word_tag(segment: &str, tag: &str) -> bool {
    let needle = format!("<{tag}");
    let Some(start) = segment.find(&needle) else {
        return false;
    };
    let tail = &segment[start..];
    let Some(end) = tail.find('>') else {
        return true;
    };
    let opening = &tail[..=end];
    !opening.contains("w:val=\"0\"")
        && !opening.contains("w:val=\"false\"")
        && !opening.contains("w:val=\"none\"")
}

fn docx_run_text(run: &str) -> String {
    let mut result = String::new();
    let mut cursor = 0usize;
    loop {
        let text_pos = run[cursor..].find("<w:t").map(|value| cursor + value);
        let tab_pos = run[cursor..].find("<w:tab").map(|value| cursor + value);
        let br_pos = run[cursor..].find("<w:br").map(|value| cursor + value);
        let next = [
            text_pos.map(|value| (value, 0u8)),
            tab_pos.map(|value| (value, 1u8)),
            br_pos.map(|value| (value, 2u8)),
        ]
        .into_iter()
        .flatten()
        .min_by_key(|(value, _)| *value);
        let Some((position, kind)) = next else {
            break;
        };
        match kind {
            0 => {
                let Some(open_rel) = run[position..].find('>') else {
                    break;
                };
                let content_start = position + open_rel + 1;
                let Some(close_rel) = run[content_start..].find("</w:t>") else {
                    break;
                };
                let content_end = content_start + close_rel;
                result.push_str(&xml_unescape(&run[content_start..content_end]));
                cursor = content_end + "</w:t>".len();
            }
            1 => {
                result.push('\t');
                cursor = run[position..]
                    .find('>')
                    .map(|value| position + value + 1)
                    .unwrap_or(run.len());
            }
            _ => {
                result.push('\n');
                cursor = run[position..]
                    .find('>')
                    .map(|value| position + value + 1)
                    .unwrap_or(run.len());
            }
        }
    }
    result
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn docx_foreground_color(value: &str) -> Option<crate::format::PaletteColor> {
    use crate::format::PaletteColor as C;
    match value.trim_start_matches('#').to_ascii_uppercase().as_str() {
        "FFFFFF" | "EBDBB2" => Some(C::White),
        "FF0000" | "FB4934" => Some(C::Red),
        "FF8000" | "FE8019" => Some(C::Orange),
        "FFFF00" | "FABD2F" => Some(C::Yellow),
        "00FF00" | "B8BB26" => Some(C::Green),
        "00FFFF" | "8EC07C" => Some(C::Cyan),
        "0000FF" | "83A598" => Some(C::Blue),
        "800080" | "FF00FF" | "D3869B" => Some(C::Purple),
        "808080" | "928374" => Some(C::Gray),
        _ => None,
    }
}

fn docx_highlight_color(value: &str) -> Option<crate::format::PaletteColor> {
    use crate::format::PaletteColor as C;
    match value.to_ascii_lowercase().as_str() {
        "yellow" => Some(C::Yellow),
        "green" => Some(C::Green),
        "cyan" | "turquoise" => Some(C::Cyan),
        "blue" => Some(C::Blue),
        "magenta" | "purple" => Some(C::Purple),
        "red" => Some(C::Red),
        "darkgray" | "lightgray" | "gray" => Some(C::Gray),
        _ => None,
    }
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

    let plain = &document.body;
    let formatting = sanitize_formatting(&document.formatting, document.body.len());
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
        body: content,
        formatting: sanitize_formatting(&formatting, usize::MAX),
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
        if run.style.strikethrough {
            object.insert("strikethrough".into(), json!(true));
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
            "runs": ranges,
            "paragraphs": []
        }),
    )
}

fn empty_formatting() -> Value {
    json!({
        "version": 1,
        "unit": "utf8-byte",
        "runs": [],
        "paragraphs": []
    })
}

fn decode_formatting_runs(formatting: &Value) -> Vec<(usize, usize, crate::format::TextStyle)> {
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
                strikethrough: value
                    .get("strikethrough")
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
            (start < end && style != crate::format::TextStyle::default())
                .then_some((start, end, style))
        })
        .collect::<Vec<_>>();
    ranges.sort_by_key(|(start, end, _)| (*start, *end));
    ranges
}

fn encode_formatting_runs(runs: &[(usize, usize, crate::format::TextStyle)]) -> Value {
    let mut encoded = Vec::new();
    for &(start, end, style) in runs {
        if start >= end || style == crate::format::TextStyle::default() {
            continue;
        }
        let mut object = Map::new();
        object.insert("start".into(), json!(start));
        object.insert("end".into(), json!(end));
        if style.bold {
            object.insert("bold".into(), json!(true));
        }
        if style.italic {
            object.insert("italic".into(), json!(true));
        }
        if style.underline {
            object.insert("underline".into(), json!(true));
        }
        if style.strikethrough {
            object.insert("strikethrough".into(), json!(true));
        }
        if let Some(color) = style.foreground {
            object.insert("foreground".into(), json!(color.name()));
        }
        if let Some(color) = style.background {
            object.insert("background".into(), json!(color.name()));
        }
        encoded.push(Value::Object(object));
    }
    json!({
        "version": 1,
        "unit": "utf8-byte",
        "runs": encoded
    })
}

fn decode_paragraph_alignments(
    formatting: &Value,
) -> Vec<(usize, usize, crate::format::ParagraphAlignment)> {
    let mut ranges = formatting
        .get("paragraphs")
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
  let alignment = value
      .get("alignment")?
      .as_str()
      .and_then(crate::format::ParagraphAlignment::parse)?;
  (start <= end).then_some((start, end, alignment))
        })
        .collect::<Vec<_>>();
    ranges.sort_by_key(|(start, end, _)| (*start, *end));
    ranges
}

fn encode_formatting_parts(
    runs: &[(usize, usize, crate::format::TextStyle)],
    paragraphs: &[(usize, usize, crate::format::ParagraphAlignment)],
) -> Value {
    let mut value = encode_formatting_runs(runs);
    let encoded = paragraphs
        .iter()
        .filter(|(start, end, alignment)| start <= end && *alignment != crate::format::ParagraphAlignment::Left)
        .map(|(start, end, alignment)| {
  json!({
      "start": start,
      "end": end,
      "alignment": alignment.name()
  })
        })
        .collect::<Vec<_>>();
    if let Some(object) = value.as_object_mut() {
        object.insert("paragraphs".into(), Value::Array(encoded));
    }
    value
}

fn paragraph_bounds(body: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(body.len());
    let start = body[..offset].rfind('\n').map(|index| index + 1).unwrap_or(0);
    let end = body[offset..]
        .find('\n')
        .map(|index| offset + index)
        .unwrap_or(body.len());
    (start, end)
}

fn normalize_paragraph_alignments(
    body: &str,
    ranges: &[(usize, usize, crate::format::ParagraphAlignment)],
) -> Vec<(usize, usize, crate::format::ParagraphAlignment)> {
    let mut result = Vec::new();
    let mut line_start = 0usize;
    for segment in body.split_inclusive('\n') {
        let line_end = line_start + segment.strip_suffix('\n').unwrap_or(segment).len();
        let probe = line_start.min(body.len().saturating_sub(1));
        let alignment = ranges
  .iter()
  .rev()
  .find(|(start, end, _)| {
      (*start <= probe && probe <= *end)
          || (line_start <= *start && *start <= line_end)
  })
  .map(|(_, _, alignment)| *alignment)
  .unwrap_or_default();
        if alignment != crate::format::ParagraphAlignment::Left {
  result.push((line_start, line_end, alignment));
        }
        line_start += segment.len();
    }
    if body.is_empty() {
        return result;
    }
    if !body.ends_with('\n') && line_start < body.len() {
        let alignment = ranges
  .iter()
  .rev()
  .find(|(start, end, _)| *start <= line_start && line_start <= *end)
  .map(|(_, _, alignment)| *alignment)
  .unwrap_or_default();
        if alignment != crate::format::ParagraphAlignment::Left {
  result.push((line_start, body.len(), alignment));
        }
    }
    result
}

fn apply_paragraph_alignment(
    body: &str,
    formatting: &Value,
    selection_start: usize,
    selection_end: usize,
    alignment: crate::format::ParagraphAlignment,
) -> Value {
    let existing = decode_paragraph_alignments(formatting);
    let first = paragraph_bounds(body, selection_start).0;
    let last = paragraph_bounds(body, selection_end.saturating_sub(1)).1;
    let mut output = Vec::new();
    let mut line_start = 0usize;
    for segment in body.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line_end = line_start + line.len();
        let current = existing
  .iter()
  .rev()
  .find(|(start, end, _)| *start <= line_start && line_start <= *end)
  .map(|(_, _, value)| *value)
  .unwrap_or_default();
        let next = if line_start <= last && line_end >= first {
  alignment
        } else {
  current
        };
        if next != crate::format::ParagraphAlignment::Left {
  output.push((line_start, line_end, next));
        }
        line_start += segment.len();
    }
    let runs = decode_formatting_runs(formatting);
    encode_formatting_parts(&runs, &output)
}

pub fn paragraph_alignments(
    document: &HsstDocument,
) -> Vec<(usize, usize, crate::format::ParagraphAlignment)> {
    normalize_paragraph_alignments(
        &document.body,
        &decode_paragraph_alignments(&document.formatting),
    )
}

fn sanitize_formatting(formatting: &Value, content_len: usize) -> Value {
    let runs = decode_formatting_runs(formatting)
        .into_iter()
        .filter(|(start, end, _)| *start < *end && *end <= content_len)
        .collect::<Vec<_>>();
    let paragraphs = decode_paragraph_alignments(formatting)
        .into_iter()
        .filter(|(start, end, _)| *start <= *end && *end <= content_len)
        .collect::<Vec<_>>();
    encode_formatting_parts(&runs, &paragraphs)
}

pub fn formatting_runs(document: &HsstDocument) -> Vec<(usize, usize, crate::format::TextStyle)> {
    decode_formatting_runs(&document.formatting)
        .into_iter()
        .filter(|(start, end, _)| {
            *start < *end
                && *end <= document.body.len()
                && document.body.is_char_boundary(*start)
                && document.body.is_char_boundary(*end)
        })
        .collect()
}

pub fn rich_body(document: &HsstDocument) -> String {
    body_from_parts(&document.body, &document.formatting)
}

fn body_from_parts(content: &str, formatting: &Value) -> String {
    let ranges = decode_formatting_runs(formatting);
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
    if style.strikethrough {
        output = format!("~~{output}~~");
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

fn style_at(
    runs: &[(usize, usize, crate::format::TextStyle)],
    offset: usize,
) -> crate::format::TextStyle {
    runs.iter()
        .find(|(start, end, _)| *start <= offset && offset < *end)
        .map(|(_, _, style)| *style)
        .unwrap_or_default()
}

fn apply_style_action(style: &mut crate::format::TextStyle, action: &str, value: &str) {
    match action {
        "bold" => style.bold = !style.bold,
        "italic" => style.italic = !style.italic,
        "underline" => style.underline = !style.underline,
        "strikethrough" => style.strikethrough = !style.strikethrough,
        "highlight" => {
            style.background = if style.background == Some(crate::format::PaletteColor::Yellow) {
                None
            } else {
                Some(crate::format::PaletteColor::Yellow)
            };
        }
        "font-color" => {
            let next = crate::format::PaletteColor::parse(value);
            style.foreground = if next.is_some() && style.foreground == next {
                None
            } else {
                next
            };
        }
        "highlight-color" => {
            let next = crate::format::PaletteColor::parse(value);
            style.background = if next.is_some() && style.background == next {
                None
            } else {
                next
            };
        }
        _ => {}
    }
}

fn normalized_runs_for_selection(
    body: &str,
    formatting: &Value,
    selection_start: usize,
    selection_end: usize,
    action: &str,
    value: &str,
) -> Vec<(usize, usize, crate::format::TextStyle)> {
    let existing = decode_formatting_runs(formatting);
    let mut boundaries = vec![0usize, body.len(), selection_start, selection_end];
    for (start, end, _) in &existing {
        boundaries.push((*start).min(body.len()));
        boundaries.push((*end).min(body.len()));
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut result: Vec<(usize, usize, crate::format::TextStyle)> = Vec::new();
    for pair in boundaries.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if start >= end || !body.is_char_boundary(start) || !body.is_char_boundary(end) {
            continue;
        }
        let mut style = style_at(&existing, start);
        if start < selection_end && end > selection_start {
            apply_style_action(&mut style, action, value);
        }
        if style == crate::format::TextStyle::default() {
            continue;
        }
        if let Some((_, previous_end, previous_style)) = result.last_mut()
            && *previous_end == start
            && *previous_style == style
        {
            *previous_end = end;
        } else {
            result.push((start, end, style));
        }
    }
    result
}

fn line_column_to_byte(body: &str, line: usize, column: usize) -> usize {
    let target_line = line.saturating_sub(1);
    let target_column = column.saturating_sub(1);
    let mut line_start = 0usize;
    for (index, segment) in body.split_inclusive('\n').enumerate() {
        if index == target_line {
            let line_body = segment.strip_suffix('\n').unwrap_or(segment);
            let relative = line_body
                .char_indices()
                .nth(target_column)
                .map(|(offset, _)| offset)
                .unwrap_or(line_body.len());
            return line_start + relative;
        }
        line_start += segment.len();
    }
    body.len()
}

pub fn apply_format_selection(
    path: &Path,
    selected_text: &str,
    cursor_line: usize,
    cursor_column: usize,
    action: &str,
    value: &str,
) -> Result<bool> {
    if !is_native_path(path) || selected_text.is_empty() {
        return Ok(false);
    }
    let mut document = read(path)?;
    let cursor = line_column_to_byte(&document.body, cursor_line, cursor_column);

    let mut best: Option<(usize, usize, usize)> = None;
    for (start, _) in document.body.match_indices(selected_text) {
        let end = start + selected_text.len();
        let distance = cursor.abs_diff(start).min(cursor.abs_diff(end));
        if best.is_none_or(|(_, _, current)| distance < current) {
            best = Some((start, end, distance));
        }
    }
    let Some((start, end, _)) = best else {
        return Ok(false);
    };

    let next = if action == "alignment" {
        let Some(alignment) = crate::format::ParagraphAlignment::parse(value) else {
            return Ok(false);
        };
        apply_paragraph_alignment(&document.body, &document.formatting, start, end, alignment)
    } else {
        let runs = normalized_runs_for_selection(
            &document.body,
            &document.formatting,
            start,
            end,
            action,
            value,
        );
        let paragraphs = decode_paragraph_alignments(&document.formatting);
        encode_formatting_parts(&runs, &paragraphs)
    };
    if next == document.formatting {
        return Ok(false);
    }
    document.formatting = next;
    write(path, &document)?;
    Ok(true)
}

fn utf16_position_to_byte(body: &str, line: usize, character: usize) -> usize {
    let mut line_start = 0usize;
    for (index, segment) in body.split_inclusive('\n').enumerate() {
        if index == line {
            let line_body = segment.strip_suffix('\n').unwrap_or(segment);
            let mut utf16 = 0usize;
            for (offset, ch) in line_body.char_indices() {
                if utf16 >= character {
                    return line_start + offset;
                }
                let next = utf16 + ch.len_utf16();
                if next > character {
                    return line_start + offset;
                }
                utf16 = next;
            }
            return line_start + line_body.len();
        }
        line_start += segment.len();
    }
    body.len()
}

#[allow(clippy::too_many_arguments)]
pub fn apply_format_lsp_range(
    path: &Path,
    current_body: &str,
    start_line: usize,
    start_character: usize,
    end_line: usize,
    end_character: usize,
    action: &str,
    value: &str,
) -> Result<bool> {
    if !is_native_path(path) {
        return Ok(false);
    }
    let mut document = read(path)?;
    if document.body != current_body {
        document.formatting = remap_formatting(&document.formatting, &document.body, current_body);
        document.body = current_body.to_owned();
    }
    let start = utf16_position_to_byte(&document.body, start_line, start_character);
    let end = utf16_position_to_byte(&document.body, end_line, end_character);
    if start >= end
        || end > document.body.len()
        || !document.body.is_char_boundary(start)
        || !document.body.is_char_boundary(end)
    {
        return Ok(false);
    }
    let next = if action == "alignment" {
        let Some(alignment) = crate::format::ParagraphAlignment::parse(value) else {
            return Ok(false);
        };
        apply_paragraph_alignment(&document.body, &document.formatting, start, end, alignment)
    } else {
        let runs = normalized_runs_for_selection(
            &document.body,
            &document.formatting,
            start,
            end,
            action,
            value,
        );
        let paragraphs = decode_paragraph_alignments(&document.formatting);
        encode_formatting_parts(&runs, &paragraphs)
    };
    if next == document.formatting {
        return Ok(false);
    }
    document.formatting = next;
    write(path, &document)?;
    Ok(true)
}
pub fn remap_formatting(formatting: &Value, old: &str, new: &str) -> Value {
    if old == new {
        return sanitize_formatting(formatting, new.len());
    }

    let mut prefix = 0usize;
    for (left, right) in old.chars().zip(new.chars()) {
        if left != right {
            break;
        }
        prefix += left.len_utf8();
    }

    let old_tail = &old[prefix..];
    let new_tail = &new[prefix..];
    let mut suffix = 0usize;
    for (left, right) in old_tail.chars().rev().zip(new_tail.chars().rev()) {
        if left != right {
            break;
        }
        suffix += left.len_utf8();
    }

    let old_edit_end = old.len().saturating_sub(suffix);
    let new_edit_end = new.len().saturating_sub(suffix);
    let delta = new_edit_end as isize - old_edit_end as isize;
    let shift = |value: usize| -> usize { value.saturating_add_signed(delta) };

    let mut remapped = Vec::new();
    for (start, end, style) in decode_formatting_runs(formatting) {
        if end <= prefix {
            remapped.push((start, end, style));
        } else if start >= old_edit_end {
            remapped.push((shift(start), shift(end), style));
        } else if start < prefix && end > old_edit_end {
            remapped.push((start, shift(end), style));
        } else {
            if start < prefix {
                remapped.push((start, prefix, style));
            }
            if end > old_edit_end {
                remapped.push((new_edit_end, shift(end), style));
            }
        }
    }

    remapped.retain(|(start, end, _)| {
        start < end
            && *end <= new.len()
            && new.is_char_boundary(*start)
            && new.is_char_boundary(*end)
    });

    let mut remapped_paragraphs = Vec::new();
    for (start, end, alignment) in decode_paragraph_alignments(formatting) {
        let mapped = if end <= prefix {
            Some((start, end, alignment))
        } else if start >= old_edit_end {
            Some((shift(start), shift(end), alignment))
        } else if start <= prefix && end >= old_edit_end {
            Some((start, shift(end), alignment))
        } else {
            Some((start.min(new.len()), shift(end).min(new.len()), alignment))
        };
        if let Some(range) = mapped {
            remapped_paragraphs.push(range);
        }
    }
    let remapped_paragraphs = normalize_paragraph_alignments(new, &remapped_paragraphs);
    encode_formatting_parts(&remapped, &remapped_paragraphs)
}

pub fn read_metadata(path: &Path) -> Option<DocumentMetadata> {
    if !is_native_path(path) {
        return None;
    }
    read(path).ok().map(|document| document.metadata)
}

pub fn parse(raw: &str, source: &Path) -> HsstDocument {
    let (metadata, legacy_body) = if let Some((frontmatter, body)) = split_frontmatter(raw) {
        let value = frontmatter.parse::<toml::Value>().ok();
        let metadata = value
            .as_ref()
            .and_then(toml::Value::as_table)
            .map(|table| metadata_from_table(table, source))
            .unwrap_or_else(|| fallback_metadata(source));
        (metadata, body.to_owned())
    } else {
        (fallback_metadata(source), raw.to_owned())
    };
    let (body, formatting) = formatting_payload(&legacy_body);
    HsstDocument {
        metadata,
        body,
        formatting,
    }
}

#[cfg(test)]
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
        rich_body(document)
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

#[cfg(test)]
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
            body: "Texto en negrita.\n".into(),
            formatting: json!({"version": 1, "unit": "utf8-byte", "runs": [{"start": 6, "end": 16, "bold": true}]}),
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
