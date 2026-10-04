from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly 1 match, found {count}")
    return text.replace(old, new, 1)

# ---------------- document.rs ----------------
p = Path("src/document.rs")
s = p.read_text(encoding="utf-8")

s = replace_once(s,
'''pub struct HsstDocument {
    pub metadata: DocumentMetadata,
    pub body: String,
}
''',
'''pub struct HsstDocument {
    pub metadata: DocumentMetadata,
    /// Texto plano que Helix edita. Nunca contiene marcas de formato HSST.
    pub body: String,
    /// Formato paralelo del contenido, almacenado en formatting.json.
    pub formatting: Value,
}
''', "add formatting field")

s = replace_once(s,
'''    let document = HsstDocument {
        metadata: DocumentMetadata::new(title),
        body: String::new(),
    };
''',
'''    let document = HsstDocument {
        metadata: DocumentMetadata::new(title),
        body: String::new(),
        formatting: empty_formatting(),
    };
''', "create native formatting")

s = replace_once(s,
'''    let (plain, formatting) = formatting_payload(&document.body);
    let manifest = manifest_json(&document.metadata);
''',
'''    let plain = &document.body;
    let formatting = sanitize_formatting(&document.formatting, document.body.len());
    let manifest = manifest_json(&document.metadata);
''', "write plain content")

s = replace_once(s,
'''        archive.start_file("content.txt", options)?;
        archive.write_all(plain.as_bytes())?;
''',
'''        archive.start_file("content.txt", options)?;
        archive.write_all(plain.as_bytes())?;
''', "confirm content write")

s = replace_once(s,
'''    Ok(HsstDocument {
        metadata: metadata_from_manifest(&manifest, path),
        body: body_from_parts(&content, &formatting),
    })
}
''',
'''    Ok(HsstDocument {
        metadata: metadata_from_manifest(&manifest, path),
        body: content,
        formatting: sanitize_formatting(&formatting, usize::MAX),
    })
}
''', "read container as plain body")

start = s.find("fn body_from_parts(content: &str, formatting: &Value) -> String {\n")
end = s.find("\npub fn read_metadata(path: &Path) -> Option<DocumentMetadata> {", start)
if start < 0 or end < 0:
    raise SystemExit("document formatting block not found")
new_block = r'''fn empty_formatting() -> Value {
    json!({
        "version": 1,
        "unit": "utf8-byte",
        "runs": []
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

fn sanitize_formatting(formatting: &Value, content_len: usize) -> Value {
    let runs = decode_formatting_runs(formatting)
        .into_iter()
        .filter(|(start, end, _)| *start < *end && *end <= content_len)
        .collect::<Vec<_>>();
    encode_formatting_runs(&runs)
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

    let runs = normalized_runs_for_selection(
        &document.body,
        &document.formatting,
        start,
        end,
        action,
        value,
    );
    let next = encode_formatting_runs(&runs);
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
    encode_formatting_runs(&remapped)
}
'''
s = s[:start] + new_block + s[end:]

# legacy parse -> migrate markup into clean body + formatting
old = '''pub fn parse(raw: &str, source: &Path) -> HsstDocument {
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
'''
new = '''pub fn parse(raw: &str, source: &Path) -> HsstDocument {
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
'''
s = replace_once(s, old, new, "legacy parse migration")

# legacy serializer test must materialize rich body
s = replace_once(s, '''        document.body
    )
}
''', '''        rich_body(document)
    )
}
''', "legacy serializer rich body")

# test initializer field
s = s.replace('''            body: "Texto **en negrita**.\\n".into(),
        };
''', '''            body: "Texto en negrita.\\n".into(),
            formatting: json!({"version": 1, "unit": "utf8-byte", "runs": [{"start": 6, "end": 16, "bold": true}]}),
        };
''')

p.write_text(s, encoding="utf-8")

# ---------------- editor.rs ----------------
p = Path("src/editor.rs")
s = p.read_text(encoding="utf-8")
s = replace_once(s,
'''struct NativeBuffer {
    source: PathBuf,
    edit: PathBuf,
    last_stamp: Option<FileStamp>,
}
''',
'''struct NativeBuffer {
    source: PathBuf,
    edit: PathBuf,
    last_stamp: Option<FileStamp>,
    last_body: String,
}
''', "native last body")

old = '''        let source = native.source.clone();
        let edit = native.edit.clone();
        let body = fs::read_to_string(&edit)
            .with_context(|| format!("No se pudo leer el cuerpo editable {}", edit.display()))?;
        let mut document = document::read(&source)?;
        let changed = document.body != body;

        if changed {
            document.body = body;
            document::write(&source, &document)
                .with_context(|| format!("No se pudo guardar {}", source.display()))?;
        }

        if let Some(native) = self.native.as_mut() {
            native.last_stamp = stamp;
        }
'''
new = '''        let source = native.source.clone();
        let edit = native.edit.clone();
        let previous_body = native.last_body.clone();
        let body = fs::read_to_string(&edit)
            .with_context(|| format!("No se pudo leer el cuerpo editable {}", edit.display()))?;
        let mut document = document::read(&source)?;
        let changed = document.body != body;

        if changed {
            document.formatting = document::remap_formatting(&document.formatting, &previous_body, &body);
            document.body = body.clone();
            document::write(&source, &document)
                .with_context(|| format!("No se pudo guardar {}", source.display()))?;
        }

        if let Some(native) = self.native.as_mut() {
            native.last_stamp = stamp;
            native.last_body = body;
        }
'''
s = replace_once(s, old, new, "remap formatting on edit")

old = '''    fs::write(&edit, document.body.as_bytes())
        .with_context(|| format!("No se pudo preparar {}", source.display()))?;
    let last_stamp = file_stamp(&edit);

    Ok(NativeBuffer {
        source: source.to_path_buf(),
        edit,
        last_stamp,
    })
'''
new = '''    fs::write(&edit, document.body.as_bytes())
        .with_context(|| format!("No se pudo preparar {}", source.display()))?;
    let last_stamp = file_stamp(&edit);
    let last_body = document.body;

    Ok(NativeBuffer {
        source: source.to_path_buf(),
        edit,
        last_stamp,
        last_body,
    })
'''
s = replace_once(s, old, new, "prepare clean native buffer")
p.write_text(s, encoding="utf-8")

# ---------------- format.rs ----------------
p = Path("src/format.rs")
s = p.read_text(encoding="utf-8")
s = s.replace('''pub fn run_filter(action: &str, value: Option<&str>) -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let output = apply_to_selection(&input, action, value.unwrap_or(""));
    std::io::stdout().write_all(output.as_bytes())?;
    Ok(())
}
''', '''pub fn run_filter(
    action: &str,
    value: Option<&str>,
    source: Option<&std::path::Path>,
    cursor_line: usize,
    cursor_column: usize,
) -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    if let Some(source) = source {
        let _ = crate::document::apply_format_selection(
            source,
            &input,
            cursor_line,
            cursor_column,
            action,
            value.unwrap_or(""),
        )?;
    }
    // Helix recibe exactamente el mismo texto: el formato vive fuera de content.txt.
    std::io::stdout().write_all(input.as_bytes())?;
    Ok(())
}
''')

old = '''pub fn pipe_command(action: &str, value: Option<&str>) -> Result<String> {
    let executable = std::env::current_exe().context("No se pudo localizar Helix-SST")?;
    let executable = executable
        .to_string_lossy()
        .replace('\\\\', "/")
        .replace('"', "\\\\\\\"");
    let mut command = format!(":pipe \\\"{executable}\\\" --hsst-format {action}");
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        command.push(' ');
        command.push_str(value);
    }
    Ok(command)
}
'''
new = '''pub fn pipe_command(
    action: &str,
    value: Option<&str>,
    source: &std::path::Path,
    cursor_line: usize,
    cursor_column: usize,
) -> Result<String> {
    let executable = std::env::current_exe().context("No se pudo localizar Helix-SST")?;
    let executable = executable
        .to_string_lossy()
        .replace('\\\\', "/")
        .replace('"', "\\\\\\\"");
    let source = source
        .to_string_lossy()
        .replace('\\\\', "/")
        .replace('"', "\\\\\\\"");
    let value = value.filter(|value| !value.is_empty()).unwrap_or("-");
    Ok(format!(
        ":pipe \\\"{executable}\\\" --hsst-format {action} {value} \\\"{source}\\\" {cursor_line} {cursor_column}"
    ))
}
'''
if old not in s:
    # Handle current file's normal quote spelling exactly.
    start = s.find('pub fn pipe_command(')
    end = s.find('\npub fn ensure_theme()', start)
    if start < 0 or end < 0:
        raise SystemExit('pipe_command block not found')
    s = s[:start] + new + s[end+1:]
else:
    s = s.replace(old, new, 1)
p.write_text(s, encoding="utf-8")

# ---------------- main.rs ----------------
p = Path("src/main.rs")
s = p.read_text(encoding="utf-8")
old = '''    if args.first().map(String::as_str) == Some("--hsst-format") {
        let action = args.get(1).map(String::as_str).unwrap_or("");
        let value = args.get(2).map(String::as_str);
        format::run_filter(action, value)?;
        return Ok(());
    }
'''
new = '''    if args.first().map(String::as_str) == Some("--hsst-format") {
        let action = args.get(1).map(String::as_str).unwrap_or("");
        let value = args
            .get(2)
            .map(String::as_str)
            .filter(|value| *value != "-");
        let source = args.get(3).map(PathBuf::from);
        let cursor_line = args.get(4).and_then(|value| value.parse().ok()).unwrap_or(1);
        let cursor_column = args.get(5).and_then(|value| value.parse().ok()).unwrap_or(1);
        format::run_filter(action, value, source.as_deref(), cursor_line, cursor_column)?;
        return Ok(());
    }
'''
s = replace_once(s, old, new, "format CLI args")
p.write_text(s, encoding="utf-8")

# ---------------- app.rs ----------------
p = Path("src/app.rs")
s = p.read_text(encoding="utf-8")
start = s.find('    fn apply_format(&mut self, action: &str, value: &str) {\n')
end = s.find('\n    fn update_page(', start)
if start < 0 or end < 0:
    raise SystemExit('apply_format block not found')
new = '''    fn apply_format(&mut self, action: &str, value: &str) {
        let Some(current) = self.current_file.clone() else {
            return;
        };
        if !document::is_native_path(&current) {
            return;
        }
        let (cursor_line, cursor_column) = self.helix_cursor_position().unwrap_or((1, 1));
        let leave_insert = self.helix_is_insert_mode();
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        if leave_insert {
            let _ = editor.send_key(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                editor.win32_input(),
            );
        }
        let value = (!value.is_empty()).then_some(value);
        match format::pipe_command(action, value, &current, cursor_line, cursor_column) {
            Ok(command) => {
                if let Err(error) = editor.send_command(&command) {
                    self.launcher.message = Some(format!("No se pudo aplicar formato: {error}"));
                }
            }
            Err(error) => {
                self.launcher.message = Some(format!("No se pudo preparar formato: {error}"));
            }
        }
    }
'''
s = s[:start] + new + s[end:]
p.write_text(s, encoding="utf-8")

# ---------------- spell.rs ----------------
p = Path("src/spell.rs")
s = p.read_text(encoding="utf-8")
needle = '''        absolute.sort_unstable();
        let mut data = Vec::with_capacity(absolute.len() * 5);
'''
insert = '''        if let Some(source) = self.source_file.as_ref()
            && crate::document::is_native_path(source)
            && let Ok(document) = crate::document::read(source)
            && document.body == *text
        {
            append_native_format_tokens(text, &document, &mut absolute);
        }

        absolute.sort_unstable();
        let mut data = Vec::with_capacity(absolute.len() * 5);
'''
s = replace_once(s, needle, insert, "append native semantic tokens")
helper_marker = '\nfn word_prefix_at(text: &str, line: usize, utf16_character: usize) -> String {\n'
helper = r'''
fn semantic_token_for_style(style: crate::format::TextStyle) -> Option<u32> {
    use crate::format::{MarkKind, StyleRange};
    let kind = if let Some(color) = style.foreground {
        MarkKind::Foreground(color)
    } else if let Some(color) = style.background {
        MarkKind::Background(color)
    } else if style.underline {
        MarkKind::Underline
    } else if style.bold {
        MarkKind::Bold
    } else if style.italic {
        MarkKind::Italic
    } else {
        return None;
    };
    Some(StyleRange { start: 0, end: 1, kind }.semantic_token())
}

fn append_native_format_tokens(
    text: &str,
    document: &crate::document::HsstDocument,
    output: &mut Vec<(u32, u32, u32, u32)>,
) {
    let mut lines = Vec::new();
    let mut offset = 0usize;
    for (line_index, segment) in text.split_inclusive('\n').enumerate() {
        let content = segment.strip_suffix('\n').unwrap_or(segment);
        lines.push((line_index as u32, offset, content));
        offset += segment.len();
    }
    if text.is_empty() || !text.ends_with('\n') {
        if lines.is_empty() {
            lines.push((0, 0, text));
        }
    }

    for (start, end, style) in crate::document::formatting_runs(document) {
        let Some(token_type) = semantic_token_for_style(style) else {
            continue;
        };
        for &(line_number, line_start, line) in &lines {
            let line_end = line_start + line.len();
            let part_start = start.max(line_start);
            let part_end = end.min(line_end);
            if part_start >= part_end
                || !text.is_char_boundary(part_start)
                || !text.is_char_boundary(part_end)
            {
                continue;
            }
            let relative_start = part_start - line_start;
            let relative_end = part_end - line_start;
            let start_utf16 = line[..relative_start].encode_utf16().count() as u32;
            let length_utf16 = line[relative_start..relative_end].encode_utf16().count() as u32;
            if length_utf16 > 0 {
                output.push((line_number, start_utf16, length_utf16, token_type));
            }
        }
    }
}
'''
if helper_marker not in s:
    raise SystemExit('spell helper marker not found')
s = s.replace(helper_marker, '\n' + helper + helper_marker, 1)
p.write_text(s, encoding="utf-8")

# ---------------- export.rs ----------------
p = Path("src/export.rs")
s = p.read_text(encoding="utf-8")
s = s.replace('''            body: document.body,
            page: document.metadata.page,
''', '''            body: document::rich_body(&document),
            page: document.metadata.page,
''')
s = s.replace('''            body: current.body,
            page: current.metadata.page,
''', '''            body: document::rich_body(&current),
            page: current.metadata.page,
''')
s = s.replace('''                body: document.body,
                page: document.metadata.page,
''', '''                body: document::rich_body(&document),
                page: document.metadata.page,
''')
p.write_text(s, encoding="utf-8")

print('HSST true compound model applied: clean content.txt + formatting.json sidecar')
