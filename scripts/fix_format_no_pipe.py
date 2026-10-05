from pathlib import Path

# editor.rs: direct selection bridge + internal save/reload keys.
p = Path('src/editor.rs')
s = p.read_text(encoding='utf-8')
old = '''    pub fn send_command(&self, command: &str) -> Result<()> {
        let win32 = self.win32_input();
        let mut bytes = Vec::new();
        let payload = if let Some(rest) = command.strip_prefix(':') {
            bytes.extend_from_slice(&encode_command_colon(win32));
            rest
        } else {
            command
        };

        for ch in payload.chars() {
            if let Some(encoded) =
                encode_input(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), win32)
            {
                bytes.extend_from_slice(&encoded);
            }
        }
        if let Some(encoded) =
            encode_input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), win32)
        {
            bytes.extend_from_slice(&encoded);
        }

        self.write_reply(&bytes)
    }
'''
new = old + '''
    pub fn yank_selection_to_clipboard(&self) -> Result<()> {
        let win32 = self.win32_input();
        let mut bytes = Vec::new();
        for code in [KeyCode::Char(' '), KeyCode::Char('y')] {
            if let Some(encoded) = encode_input(KeyEvent::new(code, KeyModifiers::NONE), win32) {
                bytes.extend_from_slice(&encoded);
            }
        }
        self.write_reply(&bytes)
    }

    pub fn save_buffer_for_formatting(&self) -> Result<()> {
        self.send_key(KeyEvent::new(KeyCode::F(20), KeyModifiers::NONE), self.win32_input())
    }

    pub fn refresh_after_formatting(&self) -> Result<()> {
        self.send_key(KeyEvent::new(KeyCode::F(21), KeyModifiers::NONE), self.win32_input())
    }
'''
if old not in s:
    raise SystemExit('send_command block not found')
s = s.replace(old, new, 1)
for section in ['[keys.normal]', '[keys.select]']:
    marker = section + '\n'
    idx = s.find(marker)
    if idx < 0:
        raise SystemExit(f'{section} not found')
    insert_at = idx + len(marker)
    s = s[:insert_at] + 'F20 = ":write"\nF21 = ":reload"\n' + s[insert_at:]
p.write_text(s, encoding='utf-8')

# app.rs: never use :pipe for formatting.
p = Path('src/app.rs')
s = p.read_text(encoding='utf-8')
old = '''    fn apply_format(&mut self, action: &str, value: &str) {
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
new = '''    fn apply_format(&mut self, action: &str, value: &str) {
        let Some(current) = self.current_file.clone() else {
            return;
        };
        if !document::is_native_path(&current) {
            return;
        }
        let (cursor_line, cursor_column) = self.helix_cursor_position().unwrap_or((1, 1));
        let leave_insert = self.helix_is_insert_mode();
        let Some(editor) = self.editor.as_mut() else {
            return;
        };

        if leave_insert {
            let _ = editor.send_key(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                editor.win32_input(),
            );
        }

        if let Err(error) = editor.save_buffer_for_formatting() {
            self.launcher.message = Some(format!("No se pudo guardar antes de formatear: {error}"));
            return;
        }
        for _ in 0..12 {
            std::thread::sleep(Duration::from_millis(15));
            if editor.sync_native().unwrap_or(false) {
                break;
            }
        }

        let sentinel = format!("__HSST_SELECTION_{}_{}__", std::process::id(), cursor_line);
        let mut clipboard = match arboard::Clipboard::new() {
            Ok(clipboard) => clipboard,
            Err(error) => {
                self.launcher.message = Some(format!("No se pudo abrir el portapapeles: {error}"));
                return;
            }
        };
        let _ = clipboard.set_text(sentinel.clone());
        if let Err(error) = editor.yank_selection_to_clipboard() {
            self.launcher.message = Some(format!("No se pudo leer la selección: {error}"));
            return;
        }

        let mut selected = None;
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(10));
            if let Ok(text) = clipboard.get_text()
                && text != sentinel
            {
                selected = Some(text);
                break;
            }
        }
        let Some(selected) = selected.filter(|text| !text.is_empty()) else {
            self.launcher.message = Some("Selecciona texto antes de aplicar formato.".into());
            return;
        };

        match document::apply_format_selection(
            &current,
            &selected,
            cursor_line,
            cursor_column,
            action,
            value,
        ) {
            Ok(true) => {
                let _ = editor.refresh_after_formatting();
                self.dirty = true;
            }
            Ok(false) => {
                self.launcher.message = Some("No se pudo localizar la selección en el documento.".into());
            }
            Err(error) => {
                self.launcher.message = Some(format!("No se pudo aplicar formato: {error}"));
            }
        }
    }
'''
if old not in s:
    raise SystemExit('apply_format block not found')
s = s.replace(old, new, 1)
p.write_text(s, encoding='utf-8')

# spell.rs: detached HSST formatting -> semantic tokens.
p = Path('src/spell.rs')
s = p.read_text(encoding='utf-8')
needle = '''    fn semantic_tokens(&self, params: &Value) -> Vec<u32> {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return Vec::new();
        };
        let Some(text) = self.documents.get(uri) else {
            return Vec::new();
        };

        let mut absolute = Vec::<(u32, u32, u32, u32)>::new();
'''
replacement = '''    fn semantic_tokens(&self, params: &Value) -> Vec<u32> {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return Vec::new();
        };
        let Some(text) = self.documents.get(uri) else {
            return Vec::new();
        };

        let mut absolute = Vec::<(u32, u32, u32, u32)>::new();
        if let Some(source) = self.source_file.as_deref()
            && crate::document::is_native_path(source)
            && let Ok(document) = crate::document::read(source)
            && document.body == *text
        {
            append_native_format_tokens(&document, &mut absolute);
        }
'''
if needle not in s:
    raise SystemExit('semantic_tokens prelude not found')
s = s.replace(needle, replacement, 1)

legacy_start = s.find('            let mut ranges = crate::format::style_ranges(line);')
if legacy_start >= 0:
    legacy_end = s.find('            }\n', s.find('                occupied_until = range.end;', legacy_start))
    if legacy_end < 0:
        raise SystemExit('legacy token block end not found')
    legacy_end += len('            }\n')
    old_block = s[legacy_start:legacy_end]
    new_block = '''            if self
                .source_file
                .as_deref()
                .is_none_or(|source| !crate::document::is_native_path(source))
            {
                let mut ranges = crate::format::style_ranges(line);
                ranges.sort_by_key(|range| {
                    (
                        range.start,
                        range.end.saturating_sub(range.start),
                        range.semantic_token(),
                    )
                });
                let mut occupied_until = 0usize;
                for range in ranges {
                    if range.start < occupied_until
                        || range.start >= range.end
                        || !line.is_char_boundary(range.start)
                        || !line.is_char_boundary(range.end)
                    {
                        continue;
                    }
                    let start_utf16 = line[..range.start].encode_utf16().count() as u32;
                    let length_utf16 = line[range.start..range.end].encode_utf16().count() as u32;
                    absolute.push((line_number, start_utf16, length_utf16, range.semantic_token()));
                    occupied_until = range.end;
                }
            }
'''
    s = s[:legacy_start] + new_block + s[legacy_end:]

insert_marker = '\nfn read_message('
helper = r'''
fn text_style_token(style: crate::format::TextStyle) -> Option<u32> {
    if let Some(color) = style.background {
        return Some(
            crate::format::StyleRange {
                start: 0,
                end: 1,
                kind: crate::format::MarkKind::Background(color),
            }
            .semantic_token(),
        );
    }
    if let Some(color) = style.foreground {
        return Some(
            crate::format::StyleRange {
                start: 0,
                end: 1,
                kind: crate::format::MarkKind::Foreground(color),
            }
            .semantic_token(),
        );
    }
    if style.underline {
        return Some(5);
    }
    if style.bold {
        return Some(2);
    }
    if style.italic {
        return Some(4);
    }
    None
}

fn append_native_format_tokens(
    document: &crate::document::HsstDocument,
    absolute: &mut Vec<(u32, u32, u32, u32)>,
) {
    let body = &document.body;
    let mut line_starts = vec![0usize];
    for (index, byte) in body.bytes().enumerate() {
        if byte == b'\n' {
            line_starts.push(index + 1);
        }
    }

    for (start, end, style) in crate::document::formatting_runs(document) {
        let Some(token) = text_style_token(style) else {
            continue;
        };
        for (line_index, &line_start) in line_starts.iter().enumerate() {
            let line_end = line_starts
                .get(line_index + 1)
                .copied()
                .unwrap_or(body.len());
            let content_end = if line_end > line_start
                && body.as_bytes().get(line_end - 1) == Some(&b'\n')
            {
                line_end - 1
            } else {
                line_end
            };
            let seg_start = start.max(line_start);
            let seg_end = end.min(content_end);
            if seg_start >= seg_end
                || !body.is_char_boundary(seg_start)
                || !body.is_char_boundary(seg_end)
            {
                continue;
            }
            let column = body[line_start..seg_start].encode_utf16().count() as u32;
            let length = body[seg_start..seg_end].encode_utf16().count() as u32;
            absolute.push((line_index as u32, column, length, token));
        }
    }
}
'''
if insert_marker not in s:
    raise SystemExit('read_message marker not found')
s = s.replace(insert_marker, '\n' + helper + insert_marker, 1)
p.write_text(s, encoding='utf-8')

# Remove obsolete formatter helper and :pipe builder.
p = Path('src/format.rs')
s = p.read_text(encoding='utf-8')
start = s.find('pub fn run_filter(')
if start >= 0:
    end = s.find('\n#[cfg(test)]\npub fn apply_to_selection', start)
    if end < 0:
        raise SystemExit('run_filter end not found')
    s = s[:start] + s[end+1:]
start = s.find('pub fn pipe_command(')
if start >= 0:
    end = s.find('\npub fn ensure_theme()', start)
    if end < 0:
        raise SystemExit('pipe_command end not found')
    s = s[:start] + s[end+1:]
s = s.replace('use std::{\n    fs,\n    io::{Read, Write},\n    path::PathBuf,\n};', 'use std::{fs, path::PathBuf};')
p.write_text(s, encoding='utf-8')

p = Path('src/main.rs')
s = p.read_text(encoding='utf-8')
start = s.find('    if args.first().map(String::as_str) == Some("--hsst-format") {')
if start >= 0:
    end = s.find('    if args.first().map(String::as_str) == Some("--clipboard-get") {', start)
    if end < 0:
        raise SystemExit('format helper branch end not found')
    s = s[:start] + s[end:]
p.write_text(s, encoding='utf-8')

p = Path('Cargo.toml')
s = p.read_text(encoding='utf-8')
if 'version = "0.2.9"' in s:
    s = s.replace('version = "0.2.9"', 'version = "0.2.10"', 1)
p.write_text(s, encoding='utf-8')
