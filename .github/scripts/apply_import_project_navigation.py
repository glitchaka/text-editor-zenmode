from pathlib import Path


def repl(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"missing marker: {label}")
    return text.replace(old, new, 1)


# ---------------- document import ----------------
p = Path("src/document.rs")
s = p.read_text(encoding="utf-8")
marker = '''pub fn create_native(path: &Path, title: &str) -> Result<()> {
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
'''
addition = marker + r'''

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
        }

        let paragraph_style =
            xml_attr_after(paragraph, "<w:pStyle", "w:val").unwrap_or_default();
        if paragraph_style.eq_ignore_ascii_case("Heading1")
            || paragraph_style.eq_ignore_ascii_case("Title")
        {
            rich.push_str("# ");
        } else if paragraph_style.eq_ignore_ascii_case("Heading2") {
            rich.push_str("## ");
        } else if paragraph_style.eq_ignore_ascii_case("Heading3") {
            rich.push_str("### ");
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
            }
            run_cursor = run_end + "</w:r>".len();
        }
        cursor = end + "</w:p>".len();
    }

    Ok(formatting_payload(&rich))
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
'''
s = repl(s, marker, addition, "document import functions")
p.write_text(s, encoding="utf-8")


# ---------------- launcher / navigation ----------------
p = Path("src/app.rs")
s = p.read_text(encoding="utf-8")
s = repl(
    s,
    '''        entries.sort_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });''',
    '''        entries.sort_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| match (&a.metadata, &b.metadata) {
                    (Some(a), Some(b)) => a
                        .project
                        .to_lowercase()
                        .cmp(&b.project.to_lowercase())
                        .then_with(|| a.order.cmp(&b.order))
                        .then_with(|| {
                            a.chapter
                                .unwrap_or(u32::MAX)
                                .cmp(&b.chapter.unwrap_or(u32::MAX))
                        })
                        .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                })
        });''',
    "logical launcher order",
)

marker = '''    fn launcher_key(&mut self, key: KeyEvent) {'''
helper = '''    fn import_selected(&mut self) -> bool {
        let Some(entry) = self.launcher.selected_entry().cloned() else {
            self.launcher.message = Some("Selecciona un TXT, Markdown o DOCX para importar.".into());
            return false;
        };
        if entry.directory || !document::is_importable_path(&entry.path) {
            self.launcher.message = Some("Formato importable: TXT, Markdown o DOCX.".into());
            return false;
        }
        match document::import_to_native(&entry.path, &self.library.documents) {
            Ok(target) => {
                self.launcher.message = Some(format!("Importado: {}", target.display()));
                self.launcher.cwd = self.library.documents.clone();
                self.launcher.refresh();
                if let Err(error) = self.open_editor(target) {
                    self.launcher.message = Some(error.to_string());
                    return false;
                }
                true
            }
            Err(error) => {
                self.launcher.message = Some(format!("No se pudo importar: {error}"));
                false
            }
        }
    }

    fn launcher_key(&mut self, key: KeyEvent) {'''
s = repl(s, marker, helper, "launcher import helper")
s = repl(
    s,
    '''            KeyCode::Char('r') | KeyCode::Char('R') => self.launcher.refresh(),

            KeyCode::Char('e') | KeyCode::Char('E') => {''',
    '''            KeyCode::Char('r') | KeyCode::Char('R') => self.launcher.refresh(),
            KeyCode::Char('i') | KeyCode::Char('I') => {
                if self.import_selected() {
                    return;
                }
            }

            KeyCode::Char('e') | KeyCode::Char('E') => {''',
    "launcher I import",
)
s = repl(
    s,
    '''                        } else if let Err(error) = self.open_editor(entry.path) {
                            self.launcher.message = Some(error.to_string());
                        }''',
    '''                        } else if entry
                            .path
                            .extension()
                            .and_then(|value| value.to_str())
                            .is_some_and(|value| value.eq_ignore_ascii_case("docx"))
                        {
                            let _ = self.import_selected();
                        } else if let Err(error) = self.open_editor(entry.path) {
                            self.launcher.message = Some(error.to_string());
                        }''',
    "enter docx import",
)
s = repl(
    s,
    '''                    "\\x1b[38;5;244m↑↓ seleccionar  Enter abrir  N nuevo  E exportar  Z modo  Backspace subir  R refrescar\\x1b[0m",''',
    '''                    "\\x1b[38;5;244m↑↓ seleccionar  Enter abrir  N nuevo  I importar  E exportar  Z modo  Backspace subir  R refrescar\\x1b[0m",''',
    "launcher help import",
)

s = repl(
    s,
    '''        if self.try_continue_to_next_chapter(key) {
            return;
        }''',
    '''        if self.try_continue_to_previous_chapter(key) || self.try_continue_to_next_chapter(key) {
            return;
        }''',
    "previous chapter call",
)
next_marker = '''    fn try_continue_to_next_chapter(&mut self, key: KeyEvent) -> bool {'''
previous_fn = r'''    fn try_continue_to_previous_chapter(&mut self, key: KeyEvent) -> bool {
        if key.code != KeyCode::Up
            || !key.modifiers.is_empty()
            || !self.helix_is_normal_mode()
            || self.helix_is_modified()
            || self
                .chapter_switch_until
                .is_some_and(|until| Instant::now() < until)
        {
            return false;
        }

        let Some(current) = self.current_file.clone() else {
            return false;
        };
        let Some(metadata) = document::read_metadata(&current) else {
            return false;
        };
        if !metadata.kind.eq_ignore_ascii_case("chapter")
            || metadata.chapter.is_none()
            || metadata.project.trim().is_empty()
            || self.helix_current_line().unwrap_or(usize::MAX) > 1
        {
            return false;
        }

        if let Some(editor) = self.editor.as_mut() {
            let _ = editor.flush_native();
        }
        let Ok(chapters) = document::project_chapters(&self.library.documents, &metadata.project)
        else {
            return false;
        };
        let Some(index) = chapters
            .iter()
            .position(|(path, _)| paths_equivalent(path, &current))
        else {
            return false;
        };
        let Some((previous, previous_metadata)) =
            index.checked_sub(1).and_then(|index| chapters.get(index))
        else {
            return false;
        };
        let Ok(previous_document) = document::read(previous) else {
            return false;
        };
        let last_line = previous_document
            .body
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        let Some(editor) = self.editor.as_mut() else {
            return false;
        };
        if editor.open_source(previous).is_err()
            || editor.send_command(&format!(":goto {last_line}")).is_err()
        {
            return false;
        }

        self.current_file = Some(previous.clone());
        self.source_modified = fs::metadata(previous)
            .and_then(|metadata| metadata.modified())
            .ok();
        self.page_profile = previous_metadata.page;
        let (cols, rows) = self.terminal_size();
        self.parser.screen_mut().set_size(rows, cols);
        if let Some(editor) = self.editor.as_mut() {
            let _ = editor.resize(cols, rows);
        }
        self.glyphs.clear();
        self.chapter_switch_until = Some(Instant::now() + Duration::from_millis(700));
        self.launcher.message = Some(format!("{} ← {}", previous_metadata.title, metadata.title));
        true
    }

    fn try_continue_to_next_chapter(&mut self, key: KeyEvent) -> bool {'''
s = repl(s, next_marker, previous_fn, "previous chapter function")
s = repl(
    s,
    '''        self.current_file = Some(next.clone());
        self.page_profile = next_metadata.page;''',
    '''        self.current_file = Some(next.clone());
        self.source_modified = fs::metadata(next)
            .and_then(|metadata| metadata.modified())
            .ok();
        self.page_profile = next_metadata.page;''',
    "next chapter source stamp",
)
p.write_text(s, encoding="utf-8")
