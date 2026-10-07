use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::PathBuf,
};

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};
use spellbook::Dictionary;

const ES_CL_AFF: &str = include_str!(concat!(env!("OUT_DIR"), "/helix-sst-es-CL.aff"));
const ES_CL_DIC: &str = include_str!(concat!(env!("OUT_DIR"), "/helix-sst-es-CL.dic"));
pub const DICTIONARY_LICENSE: &str =
    include_str!(concat!(env!("OUT_DIR"), "/helix-sst-es-CL.LICENSE"));

const SOURCE: &str = "Helix-SST ortografía";
const ADD_WORD_COMMAND: &str = "helix-sst.addWord";
const FORMAT_COMMAND: &str = "helix-sst.formatSelection";

pub fn run_lsp(
    user_dictionary: PathBuf,
    library_root: Option<PathBuf>,
    source_file: Option<PathBuf>,
) -> Result<i32> {
    let mut server = SpellServer::new(user_dictionary, library_root, source_file)?;
    let mut semantic_refresh_serial = 0u64;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = BufReader::new(stdin.lock());
    let mut output = stdout.lock();

    while let Some(message) = read_message(&mut input)? {
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let id = message.get("id").cloned();
        if method.is_empty() && (message.get("result").is_some() || message.get("error").is_some())
        {
            continue;
        }

        match method {
            "initialize" => {
                if let Some(id) = id {
                    send_response(
                        &mut output,
                        id,
                        json!({
                            "capabilities": {
                                "positionEncoding": "utf-16",
                                "textDocumentSync": 1,
                                "codeActionProvider": true,
                                "completionProvider": {
                                    "triggerCharacters": []
                                },
                                "semanticTokensProvider": {
                                    "legend": {
                                        "tokenTypes": crate::format::SEMANTIC_TOKEN_TYPES,
                                        "tokenModifiers": []
                                    },
                                    "full": true
                                },
                                "executeCommandProvider": {
                                    "commands": [ADD_WORD_COMMAND, FORMAT_COMMAND]
                                }
                            },
                            "serverInfo": {
                                "name": "helix-sst-spell",
                                "version": "0.1.0"
                            }
                        }),
                    )?;
                }
            }
            "initialized" | "$/cancelRequest" => {}
            "textDocument/didOpen" => {
                if let Some(params) = message.get("params") {
                    let uri = params.pointer("/textDocument/uri").and_then(Value::as_str);
                    let text = params.pointer("/textDocument/text").and_then(Value::as_str);
                    if let (Some(uri), Some(text)) = (uri, text) {
                        server.documents.insert(uri.to_owned(), text.to_owned());
                        server.publish(uri, text, &mut output)?;
                    }
                }
            }
            "textDocument/didChange" => {
                if let Some(params) = message.get("params")
                    && let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str)
                    && let Some(text) = params
                        .get("contentChanges")
                        .and_then(Value::as_array)
                        .and_then(|changes| changes.last())
                        .and_then(|change| change.get("text"))
                        .and_then(Value::as_str)
                {
                    server.documents.insert(uri.to_owned(), text.to_owned());
                    server.publish(uri, text, &mut output)?;
                }
            }
            "textDocument/didSave" => {
                if let Some(uri) = message
                    .pointer("/params/textDocument/uri")
                    .and_then(Value::as_str)
                    && let Some(text) = server.documents.get(uri).cloned()
                {
                    server.publish(uri, &text, &mut output)?;
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = message
                    .pointer("/params/textDocument/uri")
                    .and_then(Value::as_str)
                {
                    server.documents.remove(uri);
                    send_notification(
                        &mut output,
                        "textDocument/publishDiagnostics",
                        json!({ "uri": uri, "diagnostics": [] }),
                    )?;
                }
            }
            "textDocument/completion" => {
                if let Some(id) = id {
                    let items = server.completions(message.get("params").unwrap_or(&Value::Null));
                    send_response(&mut output, id, Value::Array(items))?;
                }
            }
            "textDocument/semanticTokens/full" => {
                if let Some(id) = id {
                    let data =
                        server.semantic_tokens(message.get("params").unwrap_or(&Value::Null));
                    send_response(&mut output, id, json!({ "data": data }))?;
                }
            }
            "textDocument/codeAction" => {
                if let Some(id) = id {
                    let actions =
                        server.code_actions(message.get("params").unwrap_or(&Value::Null));
                    send_response(&mut output, id, Value::Array(actions))?;
                }
            }
            "workspace/executeCommand" => {
                let command = message.pointer("/params/command").and_then(Value::as_str);
                let mut changed = false;
                match command {
                    Some(ADD_WORD_COMMAND) => {
                        if let Some(word) = message
                            .pointer("/params/arguments/0")
                            .and_then(Value::as_str)
                        {
                            changed = server.add_user_word(word)?;
                        }
                    }
                    Some(FORMAT_COMMAND) => {
                        changed = server.apply_format_command(
                            message.pointer("/params/arguments").unwrap_or(&Value::Null),
                        )?;
                    }
                    _ => {}
                }
                if let Some(id) = id {
                    send_response(&mut output, id, json!(changed))?;
                }
                if changed && command == Some(ADD_WORD_COMMAND) {
                    let documents = server
                        .documents
                        .iter()
                        .map(|(uri, text)| (uri.clone(), text.clone()))
                        .collect::<Vec<_>>();
                    for (uri, text) in documents {
                        server.publish(&uri, &text, &mut output)?;
                    }
                }
                if changed && command == Some(FORMAT_COMMAND) {
                    semantic_refresh_serial = semantic_refresh_serial.wrapping_add(1);
                    send_json(
                        &mut output,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": format!("helix-sst-semantic-refresh-{semantic_refresh_serial}"),
                            "method": "workspace/semanticTokens/refresh",
                            "params": null
                        }),
                    )?;
                }
            }
            "shutdown" => {
                if let Some(id) = id {
                    send_response(&mut output, id, Value::Null)?;
                }
            }
            "exit" => return Ok(0),
            _ => {
                if let Some(id) = id {
                    send_response(&mut output, id, Value::Null)?;
                }
            }
        }
    }

    Ok(0)
}

struct SpellServer {
    dictionary: Dictionary,
    user_dictionary: PathBuf,
    user_words: HashSet<String>,
    documents: HashMap<String, String>,
    library_root: Option<PathBuf>,
    source_file: Option<PathBuf>,
}

impl SpellServer {
    fn new(
        user_dictionary: PathBuf,
        library_root: Option<PathBuf>,
        source_file: Option<PathBuf>,
    ) -> Result<Self> {
        let mut dictionary = Dictionary::new(ES_CL_AFF, ES_CL_DIC)
            .map_err(|error| anyhow::anyhow!("diccionario es-CL inválido: {error}"))?;
        let mut user_words = HashSet::new();

        if let Ok(contents) = fs::read_to_string(&user_dictionary) {
            for line in contents.lines() {
                let word = line.trim();
                if word.is_empty() || word.starts_with('#') {
                    continue;
                }
                if dictionary.add(word).is_ok() {
                    user_words.insert(word.to_lowercase());
                }
            }
        }

        Ok(Self {
            dictionary,
            user_dictionary,
            user_words,
            documents: HashMap::new(),
            library_root,
            source_file,
        })
    }

    fn publish(&self, uri: &str, text: &str, output: &mut impl Write) -> Result<()> {
        send_notification(
            output,
            "textDocument/publishDiagnostics",
            json!({
                "uri": uri,
                "diagnostics": self.diagnostics(text)
            }),
        )
    }

    fn diagnostics(&self, text: &str) -> Vec<Value> {
        let mut diagnostics = Vec::new();
        let mut fenced = false;
        let mut frontmatter = false;
        let fence = "\x60\x60\x60";

        for (line_number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed == "+++" {
                frontmatter = !frontmatter;
                continue;
            }
            if frontmatter {
                continue;
            }
            if trimmed.starts_with(fence) || trimmed.starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }

            let ignored = ignored_spans(line);
            for (start, end, word) in word_ranges(line) {
                if ignored
                    .iter()
                    .any(|(left, right)| start >= *left && start < *right)
                {
                    continue;
                }
                if !self.should_check(&word) || self.is_correct(&word) {
                    continue;
                }

                diagnostics.push(json!({
                    "range": {
                        "start": {
                            "line": line_number,
                            "character": line[..start].encode_utf16().count()
                        },
                        "end": {
                            "line": line_number,
                            "character": line[..end].encode_utf16().count()
                        }
                    },
                    "severity": 2,
                    "source": SOURCE,
                    "message": format!("Posible error ortográfico: «{word}»"),
                    "data": { "word": word }
                }));
            }
        }

        diagnostics
    }

    fn should_check(&self, word: &str) -> bool {
        let letters = word.chars().filter(|ch| ch.is_alphabetic()).count();
        if letters < 2 {
            return false;
        }
        if word.chars().any(|ch| ch.is_ascii_digit() || ch == '_') {
            return false;
        }
        if word
            .chars()
            .all(|ch| !ch.is_alphabetic() || ch.is_uppercase())
            && letters <= 6
        {
            return false;
        }
        true
    }

    fn is_correct(&self, word: &str) -> bool {
        self.user_words.contains(&word.to_lowercase())
            || self.dictionary.check(word)
            || self.dictionary.check(&word.to_lowercase())
    }

    fn code_actions(&self, params: &Value) -> Vec<Value> {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return Vec::new();
        };
        let diagnostics = params
            .pointer("/context/diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut actions = Vec::new();
        for diagnostic in diagnostics {
            if diagnostic.get("source").and_then(Value::as_str) != Some(SOURCE) {
                continue;
            }
            let Some(word) = diagnostic.pointer("/data/word").and_then(Value::as_str) else {
                continue;
            };
            let Some(range) = diagnostic.get("range").cloned() else {
                continue;
            };

            let mut suggestions = Vec::new();
            self.dictionary.suggest(word, &mut suggestions);
            suggestions.truncate(6);

            for suggestion in suggestions {
                let mut changes = Map::new();
                changes.insert(
                    uri.to_owned(),
                    json!([{
                        "range": range.clone(),
                        "newText": suggestion
                    }]),
                );
                actions.push(json!({
                    "title": format!("Cambiar «{word}» por «{suggestion}»"),
                    "kind": "quickfix",
                    "diagnostics": [diagnostic.clone()],
                    "edit": {
                        "changes": Value::Object(changes)
                    }
                }));
            }
            actions.push(json!({
                "title": format!("Aceptar «{word}» en Helix-SST"),
                "kind": "quickfix",
                "diagnostics": [diagnostic],
                "command": {
                    "title": "Aceptar palabra",
                    "command": ADD_WORD_COMMAND,
                    "arguments": [word]
                }
            }));
        }
        if let Some(source) = self.source_file.as_ref()
            && crate::document::is_native_path(source)
            && let Some(range) = params.get("range").cloned()
            && range.pointer("/start") != range.pointer("/end")
        {
            let mut push_format = |title: &str, action: &str, value: &str| {
                actions.push(json!({
                    "title": title,
                    "kind": "refactor.rewrite",
                    "command": {
                        "title": title,
                        "command": FORMAT_COMMAND,
                        "arguments": [uri, range.clone(), action, value]
                    }
                }));
            };
            push_format("Formato · Negrita", "bold", "");
            push_format("Formato · Cursiva", "italic", "");
            push_format("Formato · Subrayado", "underline", "");
            for (label, value) in [
                ("Blanco", "white"),
                ("Rojo", "red"),
                ("Naranjo", "orange"),
                ("Amarillo", "yellow"),
                ("Verde", "green"),
                ("Cian", "cyan"),
                ("Azul", "blue"),
                ("Púrpura", "purple"),
                ("Gris", "gray"),
            ] {
                push_format(&format!("Color de texto · {label}"), "font-color", value);
            }
            push_format("Color de texto · Quitar", "font-color", "none");
            for (label, value) in [
                ("Amarillo", "yellow"),
                ("Naranjo", "orange"),
                ("Verde", "green"),
                ("Cian", "cyan"),
                ("Azul", "blue"),
                ("Púrpura", "purple"),
                ("Rojo", "red"),
                ("Gris", "gray"),
            ] {
                push_format(&format!("Resaltado · {label}"), "highlight-color", value);
            }
            push_format("Resaltado · Quitar", "highlight-color", "none");
        }
        actions
    }

    fn apply_format_command(&self, arguments: &Value) -> Result<bool> {
        let Some(source) = self.source_file.as_deref() else {
            return Ok(false);
        };
        let Some(uri) = arguments.get(0).and_then(Value::as_str) else {
            return Ok(false);
        };
        let Some(current_text) = self.documents.get(uri) else {
            return Ok(false);
        };
        let Some(range) = arguments.get(1) else {
            return Ok(false);
        };
        let action = arguments.get(2).and_then(Value::as_str).unwrap_or("");
        let value = arguments.get(3).and_then(Value::as_str).unwrap_or("");
        let start_line = range
            .pointer("/start/line")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let start_character = range
            .pointer("/start/character")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let end_line = range
            .pointer("/end/line")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let end_character = range
            .pointer("/end/character")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        crate::document::apply_format_lsp_range(
            source,
            current_text,
            start_line,
            start_character,
            end_line,
            end_character,
            action,
            value,
        )
    }

    fn completions(&self, params: &Value) -> Vec<Value> {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return Vec::new();
        };
        let Some(text) = self.documents.get(uri) else {
            return Vec::new();
        };
        let line = params
            .pointer("/position/line")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0);
        let character = params
            .pointer("/position/character")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0);
        let prefix = word_prefix_at(text, line, character);
        if prefix.chars().count() < 2 {
            return Vec::new();
        }

        let mut candidates = std::collections::BTreeSet::<String>::new();
        for (_, _, word) in text.lines().flat_map(word_ranges) {
            if starts_with_case_insensitive(&word, &prefix) && !word.eq_ignore_ascii_case(&prefix) {
                candidates.insert(word);
            }
        }

        if let Some(root) = self.library_root.as_ref() {
            collect_library_words(
                root,
                self.source_file.as_deref(),
                text,
                &prefix,
                &mut candidates,
            );
        }

        let mut spelling = Vec::new();
        self.dictionary.suggest(&prefix, &mut spelling);
        for suggestion in spelling.into_iter().take(8) {
            candidates.insert(suggestion);
        }

        candidates
            .into_iter()
            .take(20)
            .map(|label| {
                json!({
                    "label": label,
                    "kind": 1
                })
            })
            .collect()
    }

    fn semantic_tokens(&self, params: &Value) -> Vec<u32> {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return Vec::new();
        };
        let Some(text) = self.documents.get(uri) else {
            return Vec::new();
        };

        let mut absolute = Vec::<(u32, u32, u32, u32)>::new();
        let mut frontmatter = false;

        for (line_index, line) in text.lines().enumerate() {
            let line_number = line_index as u32;
            let trimmed = line.trim_start();
            let leading = line.len().saturating_sub(trimmed.len());

            if trimmed == "+++" {
                absolute.push((line_number, leading as u32, 3, 0));
                frontmatter = !frontmatter;
                continue;
            }
            if frontmatter {
                absolute.push((line_number, 0, line.encode_utf16().count() as u32, 0));
                continue;
            }
            if trimmed.starts_with("# ") {
                absolute.push((
                    line_number,
                    leading as u32,
                    trimmed.encode_utf16().count() as u32,
                    1,
                ));
                continue;
            }

            if self
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
                    absolute.push((
                        line_number,
                        start_utf16,
                        length_utf16,
                        range.semantic_token(),
                    ));
                    occupied_until = range.end;
                }
            }
        }

        if let Some(source) = self.source_file.as_ref()
            && crate::document::is_native_path(source)
            && let Ok(document) = crate::document::read(source)
            && document.body == *text
        {
            append_native_format_tokens(text, &document, &mut absolute);
        }

        absolute.sort_unstable();
        let mut data = Vec::with_capacity(absolute.len() * 5);
        let mut previous_line = 0u32;
        let mut previous_start = 0u32;

        for (line, start, length, token_type) in absolute {
            let delta_line = line.saturating_sub(previous_line);
            let delta_start = if delta_line == 0 {
                start.saturating_sub(previous_start)
            } else {
                start
            };
            data.extend_from_slice(&[delta_line, delta_start, length, token_type, 0]);
            previous_line = line;
            previous_start = start;
        }
        data
    }

    fn add_user_word(&mut self, word: &str) -> Result<bool> {
        let word = word.trim();
        if word.is_empty()
            || word.contains('\n')
            || word.contains('\r')
            || word.chars().any(char::is_whitespace)
        {
            return Ok(false);
        }

        let normalized = word.to_lowercase();
        if self.user_words.contains(&normalized) || self.dictionary.check(word) {
            return Ok(false);
        }

        if let Some(parent) = self.user_dictionary.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.user_dictionary)
            .with_context(|| format!("no se pudo abrir {}", self.user_dictionary.display()))?;
        writeln!(file, "{word}")?;

        self.dictionary
            .add(word)
            .map_err(|error| anyhow::anyhow!("no se pudo agregar «{word}»: {error}"))?;
        self.user_words.insert(normalized);
        Ok(true)
    }
}

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
    Some(
        StyleRange {
            start: 0,
            end: 1,
            kind,
        }
        .semantic_token(),
    )
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
    if (text.is_empty() || !text.ends_with('\n')) && lines.is_empty() {
        lines.push((0, 0, text));
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

fn word_prefix_at(text: &str, line: usize, utf16_character: usize) -> String {
    let Some(line) = text.lines().nth(line) else {
        return String::new();
    };

    let mut utf16 = 0usize;
    let mut byte_index = line.len();
    for (index, ch) in line.char_indices() {
        let next = utf16 + ch.len_utf16();
        if next > utf16_character {
            byte_index = index;
            break;
        }
        utf16 = next;
        byte_index = index + ch.len_utf8();
        if utf16 == utf16_character {
            break;
        }
    }

    let prefix = &line[..byte_index.min(line.len())];
    let start = prefix
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_alphabetic() && !matches!(ch, '\'' | '’'))
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    prefix[start..].to_owned()
}

fn starts_with_case_insensitive(word: &str, prefix: &str) -> bool {
    word.to_lowercase().starts_with(&prefix.to_lowercase())
}

fn collect_library_words(
    root: &std::path::Path,
    source_file: Option<&std::path::Path>,
    current_text: &str,
    prefix: &str,
    output: &mut std::collections::BTreeSet<String>,
) {
    let current_project = source_file
        .and_then(crate::document::read_metadata)
        .map(|metadata| metadata.project)
        .unwrap_or_else(|| {
            crate::document::parse(current_text, std::path::Path::new("current.hsst"))
                .metadata
                .project
        });

    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !crate::document::is_native_path(&path) {
            continue;
        }
        let Ok(document) = crate::document::read(&path) else {
            continue;
        };
        if !current_project.trim().is_empty()
            && !document
                .metadata
                .project
                .eq_ignore_ascii_case(&current_project)
        {
            continue;
        }
        for (_, _, word) in document.body.lines().flat_map(word_ranges) {
            if starts_with_case_insensitive(&word, prefix) {
                output.insert(word);
            }
        }
    }
}

fn word_ranges(line: &str) -> Vec<(usize, usize, String)> {
    let mut ranges = Vec::new();
    let mut start = None;

    for (index, ch) in line.char_indices() {
        let belongs = ch.is_alphabetic() || matches!(ch, '\'' | '’');
        match (start, belongs) {
            (None, true) => start = Some(index),
            (Some(begin), false) => {
                push_word(line, begin, index, &mut ranges);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(begin) = start {
        push_word(line, begin, line.len(), &mut ranges);
    }

    ranges
}

fn push_word(line: &str, mut start: usize, mut end: usize, out: &mut Vec<(usize, usize, String)>) {
    while start < end {
        let Some(ch) = line[start..end].chars().next() else {
            break;
        };
        if matches!(ch, '\'' | '’') {
            start += ch.len_utf8();
        } else {
            break;
        }
    }
    while start < end {
        let Some(ch) = line[start..end].chars().next_back() else {
            break;
        };
        if matches!(ch, '\'' | '’') {
            end -= ch.len_utf8();
        } else {
            break;
        }
    }
    if start < end {
        out.push((start, end, line[start..end].to_owned()));
    }
}

fn ignored_spans(line: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let tick = char::from(96);

    let mut cursor = 0usize;
    while let Some(relative) = line[cursor..].find(tick) {
        let open = cursor + relative;
        let after_open = open + tick.len_utf8();
        if let Some(relative_close) = line[after_open..].find(tick) {
            let close = after_open + relative_close + tick.len_utf8();
            spans.push((open, close));
            cursor = close;
        } else {
            spans.push((open, line.len()));
            break;
        }
    }

    for marker in ["https://", "http://", "www."] {
        let mut cursor = 0usize;
        while let Some(found) = line[cursor..].find(marker) {
            let start = cursor + found;
            let end = line[start..]
                .find(char::is_whitespace)
                .map(|offset| start + offset)
                .unwrap_or(line.len());
            spans.push((start, end));
            cursor = end;
        }
    }

    let mut cursor = 0usize;
    while let Some(open) = line[cursor..].find("](") {
        let start = cursor + open + 2;
        if let Some(close) = line[start..].find(')') {
            let end = start + close + 1;
            spans.push((start, end));
            cursor = end;
        } else {
            break;
        }
    }

    spans
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some(value) = header
            .split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .map(|(_, value)| value.trim())
        {
            content_length = Some(value.parse::<usize>().context("Content-Length inválido")?);
        }
    }

    let length = content_length.context("mensaje LSP sin Content-Length")?;
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload)?;
    Ok(Some(
        serde_json::from_slice(&payload).context("JSON LSP inválido")?,
    ))
}

fn send_response(output: &mut impl Write, id: Value, result: Value) -> Result<()> {
    send_json(
        output,
        &json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        }),
    )
}

fn send_notification(output: &mut impl Write, method: &str, params: Value) -> Result<()> {
    send_json(
        output,
        &json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }),
    )
}

fn send_json(output: &mut impl Write, value: &Value) -> Result<()> {
    let payload = serde_json::to_vec(value)?;
    write!(output, "Content-Length: {}\r\n\r\n", payload.len())?;
    output.write_all(&payload)?;
    output.flush()?;
    Ok(())
}
