from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"No se encontró bloque: {label}")
    return text.replace(old, new, 1)

# ---------------- document.rs: formato por rango LSP ----------------
p = Path("src/document.rs")
s = p.read_text(encoding="utf-8")
marker = "pub fn remap_formatting(formatting: &Value, old: &str, new: &str) -> Value {"
helper = r'''fn utf16_position_to_byte(body: &str, line: usize, character: usize) -> usize {
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

pub fn apply_format_lsp_range(
    path: &Path,
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
    let start = utf16_position_to_byte(&document.body, start_line, start_character);
    let end = utf16_position_to_byte(&document.body, end_line, end_character);
    if start >= end
        || end > document.body.len()
        || !document.body.is_char_boundary(start)
        || !document.body.is_char_boundary(end)
    {
        return Ok(false);
    }

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

'''
if helper not in s:
    s = replace_once(s, marker, helper + marker, "document remap marker")
p.write_text(s, encoding="utf-8")

# ---------------- spell.rs: F2 / Space+a para formato ----------------
p = Path("src/spell.rs")
s = p.read_text(encoding="utf-8")
s = replace_once(
    s,
    'const ADD_WORD_COMMAND: &str = "helix-sst.addWord";\n',
    'const ADD_WORD_COMMAND: &str = "helix-sst.addWord";\nconst FORMAT_COMMAND: &str = "helix-sst.formatSelection";\n',
    "FORMAT_COMMAND const",
)
s = replace_once(
    s,
    '        let id = message.get("id").cloned();\n\n        match method {',
    '        let id = message.get("id").cloned();\n        if method.is_empty() && (message.get("result").is_some() || message.get("error").is_some()) {\n            continue;\n        }\n\n        match method {',
    "ignore LSP responses",
)
s = replace_once(
    s,
    '                                    "commands": [ADD_WORD_COMMAND]\n',
    '                                    "commands": [ADD_WORD_COMMAND, FORMAT_COMMAND]\n',
    "executeCommandProvider",
)
old_exec = r'''            "workspace/executeCommand" => {
                let mut added = false;
                if message.pointer("/params/command").and_then(Value::as_str)
                    == Some(ADD_WORD_COMMAND)
                    && let Some(word) = message
                        .pointer("/params/arguments/0")
                        .and_then(Value::as_str)
                {
                    added = server.add_user_word(word)?;
                }
                if let Some(id) = id {
                    send_response(&mut output, id, json!(added))?;
                }
                if added {
                    let documents = server
                        .documents
                        .iter()
                        .map(|(uri, text)| (uri.clone(), text.clone()))
                        .collect::<Vec<_>>();
                    for (uri, text) in documents {
                        server.publish(&uri, &text, &mut output)?;
                    }
                }
            }
'''
new_exec = r'''            "workspace/executeCommand" => {
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
                } else if changed && command == Some(FORMAT_COMMAND) {
                    let request_id = server.next_request_id;
                    server.next_request_id = server.next_request_id.saturating_add(1);
                    send_request(
                        &mut output,
                        json!(request_id),
                        "workspace/semanticTokens/refresh",
                        Value::Null,
                    )?;
                }
            }
'''
s = replace_once(s, old_exec, new_exec, "executeCommand")
s = replace_once(
    s,
    '    source_file: Option<PathBuf>,\n}',
    '    source_file: Option<PathBuf>,\n    next_request_id: u64,\n}',
    "SpellServer field",
)
s = replace_once(
    s,
    '            source_file,\n        })',
    '            source_file,\n            next_request_id: 1,\n        })',
    "SpellServer init",
)
old_actions_end = '''        actions\n    }\n\n    fn completions'''
new_actions_end = r'''        if let Some(source) = self.source_file.as_ref()
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
                        "arguments": [range.clone(), action, value]
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
        }
        actions
    }

    fn apply_format_command(&self, arguments: &Value) -> Result<bool> {
        let Some(source) = self.source_file.as_deref() else {
            return Ok(false);
        };
        if !crate::document::is_native_path(source) {
            return Ok(false);
        }
        let Some(range) = arguments.get(0) else {
            return Ok(false);
        };
        let action = arguments.get(1).and_then(Value::as_str).unwrap_or("");
        let value = arguments.get(2).and_then(Value::as_str).unwrap_or("");
        let start_line = range.pointer("/start/line").and_then(Value::as_u64).unwrap_or(0) as usize;
        let start_character = range.pointer("/start/character").and_then(Value::as_u64).unwrap_or(0) as usize;
        let end_line = range.pointer("/end/line").and_then(Value::as_u64).unwrap_or(0) as usize;
        let end_character = range.pointer("/end/character").and_then(Value::as_u64).unwrap_or(0) as usize;
        crate::document::apply_format_lsp_range(
            source,
            start_line,
            start_character,
            end_line,
            end_character,
            action,
            value,
        )
    }

    fn completions'''
s = replace_once(s, old_actions_end, new_actions_end, "code actions end")
old_notify = '''fn send_notification(output: &mut impl Write, method: &str, params: Value) -> Result<()> {\n'''
new_notify = '''fn send_request(output: &mut impl Write, id: Value, method: &str, params: Value) -> Result<()> {\n    send_json(\n        output,\n        &json!({\n            "jsonrpc": "2.0",\n            "id": id,\n            "method": method,\n            "params": params\n        }),\n    )\n}\n\nfn send_notification(output: &mut impl Write, method: &str, params: Value) -> Result<()> {\n'''
s = replace_once(s, old_notify, new_notify, "send_request helper")
p.write_text(s, encoding="utf-8")

# ---------------- editor.rs: tema de descanso y ayuda F4 ----------------
p = Path("src/editor.rs")
s = p.read_text(encoding="utf-8")
s = replace_once(
    s,
    '    write_zen_theme(&themes_dir.join("helix-sst-zen.toml"))?;\n',
    '    write_zen_theme(&themes_dir.join("helix-sst-zen.toml"))?;\n    write_break_theme(&themes_dir.join("helix-sst-break.toml"))?;\n',
    "install break theme",
)
s = s.replace(
    'normal = "NORMAL · i: escribir · F2: ortografía"',
    'normal = "NORMAL · i: escribir · F2: acciones · F4: Pomodoro"',
    1,
)
s = s.replace(
    'insert = "INSERTAR · Alt+d: — · F2: ortografía · Esc: comandos"',
    'insert = "INSERTAR · Alt+d: — · F2: acciones · F4: Pomodoro · Esc: comandos"',
    1,
)
zen_end = '''fn write_language_config(\n'''
break_fn = r'''fn write_break_theme(path: &Path) -> Result<()> {
    fs::write(
        path,
        r##"inherits = "helix-sst-zen"

"ui.background" = { bg = "#2b2027" }
"ui.text" = "#f5d0c5"
"ui.text.focus" = "#ffe5dc"
"ui.cursor" = { fg = "#2b2027", bg = "#ffb4a2" }
"ui.cursor.primary" = { fg = "#2b2027", bg = "#ffb4a2" }
"ui.cursorline.primary" = { bg = "#35262d" }
"ui.selection" = { bg = "#5a3a46" }
"ui.linenr" = "#b9878d"
"ui.linenr.selected" = "#ffb4a2"
"ui.statusline" = { fg = "#ffd6c9", bg = "#3b2932" }
"ui.statusline.inactive" = { fg = "#c9939c", bg = "#32242b" }
"ui.popup" = { fg = "#ffe0d7", bg = "#3b2932" }
"ui.menu" = { fg = "#ffe0d7", bg = "#3b2932" }
"ui.menu.selected" = { fg = "#2b2027", bg = "#ffb4a2" }
"##,
    )?;
    Ok(())
}

'''
if break_fn not in s:
    s = replace_once(s, zen_end, break_fn + zen_end, "break theme function")
p.write_text(s, encoding="utf-8")

# ---------------- app.rs: Pomodoro 25/5, prompt, F4 y :PD ----------------
p = Path("src/app.rs")
s = p.read_text(encoding="utf-8")
s = replace_once(
    s,
    'const CONTENT_TOP_GAP: f32 = 8.0;\n',
    'const CONTENT_TOP_GAP: f32 = 8.0;\nconst POMODORO_WRITE_SECS: u64 = 25 * 60;\nconst POMODORO_BREAK_SECS: u64 = 5 * 60;\n',
    "pomodoro constants",
)
s = replace_once(
    s,
    '        in property <bool> editor-active: false;\n',
    '        in property <bool> editor-active: false;\n        in property <bool> pomodoro-prompt: false;\n        in property <bool> pomodoro-break: false;\n        in property <string> pomodoro-text: "";\n',
    "slint pomodoro properties",
)
# Timer pill inside island, between version and formatting controls.
version_block = '''            Text {\n                x: 112px;\n                y: 0;\n                width: 58px;\n                height: parent.height;\n                text: root.version-text;\n                color: #7f8b9b;\n                font-family: "Segoe UI Variable";\n                font-size: 12px;\n                vertical-alignment: center;\n            }\n'''
timer_pill = version_block + '''\n            Rectangle {\n                visible: root.editor-active && root.pomodoro-text != "";\n                x: island.width - 350px;\n                y: 4px;\n                width: 116px;\n                height: 26px;\n                border-radius: 8px;\n                background: root.pomodoro-break ? #5a3a46 : #172334;\n                Text {\n                    width: 100%; height: 100%; text: root.pomodoro-text;\n                    color: root.pomodoro-break ? #ffb4a2 : #e8cc83;\n                    font-family: "Segoe UI Variable"; font-size: 10px; font-weight: 650;\n                    horizontal-alignment: center; vertical-alignment: center;\n                }\n            }\n'''
s = replace_once(s, version_block, timer_pill, "pomodoro timer pill")
# Prompt overlay before the end of ZenWindow.
end_marker = '\n    }\n}\n\n#[derive(Clone, Copy)]\nstruct Rgb'
overlay = r'''

        Rectangle {
            visible: root.pomodoro-prompt;
            x: 0; y: 0; width: 100%; height: 100%;
            background: rgba(8, 10, 14, 0.72);
            Rectangle {
                width: 430px; height: 150px;
                x: (parent.width - self.width) / 2;
                y: (parent.height - self.height) / 2;
                border-radius: 12px;
                background: #171b20;
                border-width: 1px;
                border-color: #5a6570;
                Text {
                    x: 20px; y: 18px; width: parent.width - 40px; height: 28px;
                    text: "¿INICIAR POMODORO?"; color: #e8cc83;
                    font-family: "Segoe UI Variable"; font-size: 16px; font-weight: 700;
                    horizontal-alignment: center; vertical-alignment: center;
                }
                Text {
                    x: 20px; y: 54px; width: parent.width - 40px; height: 28px;
                    text: "25 min escritura · 5 min descanso"; color: #dfe8ef;
                    font-family: "Segoe UI Variable"; font-size: 13px;
                    horizontal-alignment: center; vertical-alignment: center;
                }
                Text {
                    x: 20px; y: 98px; width: parent.width - 40px; height: 28px;
                    text: "Enter / F4 iniciar     ·     Esc continuar sin temporizador";
                    color: #8db9bb; font-family: "Segoe UI Variable"; font-size: 11px;
                    horizontal-alignment: center; vertical-alignment: center;
                }
            }
        }
'''
if end_marker not in s:
    raise SystemExit("No se encontró cierre de ZenWindow")
s = s.replace(end_marker, overlay + end_marker, 1)
# TerminalModel fields.
s = replace_once(
    s,
    '    zen_requested: bool,\n    page_profile: crate::page::PageProfile,\n',
    '    zen_requested: bool,\n    pomodoro_offer: bool,\n    pomodoro_active: bool,\n    pomodoro_break: bool,\n    pomodoro_deadline: Option<Instant>,\n    command_capture: Option<String>,\n    page_profile: crate::page::PageProfile,\n',
    "TerminalModel fields",
)
s = replace_once(
    s,
    '            zen_requested,\n            page_profile: crate::page::PageProfile::default(),\n',
    '            zen_requested,\n            pomodoro_offer: false,\n            pomodoro_active: false,\n            pomodoro_break: false,\n            pomodoro_deadline: None,\n            command_capture: None,\n            page_profile: crate::page::PageProfile::default(),\n',
    "TerminalModel init",
)
# Methods after zen toggle.
zen_method = r'''    fn toggle_editor_zen(&mut self) {
        if self.editor.is_some() {
            self.zen_requested = !self.zen_requested;
            self.dirty = true;
            self.glyphs.clear();
        }
    }
'''
pomo_methods = zen_method + r'''

    fn start_pomodoro(&mut self) {
        if self.editor.is_none() || self.pomodoro_active {
            self.pomodoro_offer = false;
            return;
        }
        self.pomodoro_offer = false;
        self.pomodoro_active = true;
        self.pomodoro_break = false;
        self.pomodoro_deadline = Some(Instant::now() + Duration::from_secs(POMODORO_WRITE_SECS));
        if let Some(editor) = self.editor.as_ref() {
            let _ = editor.send_command(":theme helix-sst-zen");
        }
        self.dirty = true;
    }

    fn dismiss_pomodoro_offer(&mut self) {
        self.pomodoro_offer = false;
    }

    fn pomodoro_label(&self) -> String {
        if !self.pomodoro_active {
            return String::new();
        }
        let remaining = self
            .pomodoro_deadline
            .map(|deadline| deadline.saturating_duration_since(Instant::now()).as_secs())
            .unwrap_or(0);
        let minutes = remaining / 60;
        let seconds = remaining % 60;
        let phase = if self.pomodoro_break { "DESCANSO" } else { "ESCRITURA" };
        format!("{phase} {minutes:02}:{seconds:02}")
    }

    fn begin_pomodoro_break(&mut self) {
        self.pomodoro_break = true;
        self.pomodoro_deadline = Some(Instant::now() + Duration::from_secs(POMODORO_BREAK_SECS));
        self.command_capture = None;
        if let Some(editor) = self.editor.as_ref() {
            let win32 = editor.win32_input();
            let _ = editor.send_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), win32);
            let _ = editor.send_command(":theme helix-sst-break");
        }
        self.dirty = true;
    }

    fn begin_pomodoro_writing(&mut self) {
        self.pomodoro_break = false;
        self.pomodoro_deadline = Some(Instant::now() + Duration::from_secs(POMODORO_WRITE_SECS));
        if let Some(editor) = self.editor.as_ref() {
            let _ = editor.send_command(":theme helix-sst-zen");
        }
        self.dirty = true;
    }

    fn update_pomodoro(&mut self) {
        if !self.pomodoro_active {
            return;
        }
        let Some(deadline) = self.pomodoro_deadline else {
            return;
        };
        if Instant::now() < deadline {
            return;
        }
        if self.pomodoro_break {
            self.begin_pomodoro_writing();
        } else {
            self.begin_pomodoro_break();
        }
    }

    fn capture_pd_command(&mut self, key: KeyEvent) -> bool {
        if self.command_capture.is_none() {
            if key.code == KeyCode::Char(':')
                && key.modifiers.is_empty()
                && self.helix_is_normal_mode()
            {
                self.command_capture = Some(String::new());
            }
            return false;
        }

        match key.code {
            KeyCode::Enter => {
                let is_pd = self
                    .command_capture
                    .as_deref()
                    .is_some_and(|command| command.trim().eq_ignore_ascii_case("pd"));
                self.command_capture = None;
                if is_pd {
                    if let Some(editor) = self.editor.as_ref() {
                        let _ = editor.send_key(
                            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                            editor.win32_input(),
                        );
                    }
                    self.start_pomodoro();
                    return true;
                }
            }
            KeyCode::Esc => self.command_capture = None,
            KeyCode::Backspace => {
                if let Some(command) = self.command_capture.as_mut() {
                    command.pop();
                }
            }
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(command) = self.command_capture.as_mut() {
                    command.push(ch);
                }
            }
            _ => {}
        }
        false
    }
'''
s = replace_once(s, zen_method, pomo_methods, "pomodoro methods")
# Offer after open.
s = replace_once(
    s,
    '        self.editor = Some(session);\n        self.glyphs.clear();\n',
    '        self.editor = Some(session);\n        self.pomodoro_offer = true;\n        self.pomodoro_active = false;\n        self.pomodoro_break = false;\n        self.pomodoro_deadline = None;\n        self.command_capture = None;\n        self.glyphs.clear();\n',
    "pomodoro offer on open",
)
# Guard formatting and symbol insertion during break.
s = replace_once(
    s,
    '    fn apply_format(&mut self, action: &str, value: &str) {\n        let Some(current)',
    '    fn apply_format(&mut self, action: &str, value: &str) {\n        if self.pomodoro_break {\n            return;\n        }\n        let Some(current)',
    "format break guard",
)
s = replace_once(
    s,
    '    fn insert_symbol(&mut self, symbol: &str) {\n        if symbol.is_empty() {',
    '    fn insert_symbol(&mut self, symbol: &str) {\n        if self.pomodoro_break || symbol.is_empty() {',
    "symbol break guard",
)
# Tick timer.
s = replace_once(
    s,
    '        if self.splash_active {\n            return;\n        }\n        let mut finished = false;\n',
    '        if self.splash_active {\n            return;\n        }\n        self.update_pomodoro();\n        let mut finished = false;\n',
    "tick pomodoro",
)
# Reset on editor finish.
s = replace_once(
    s,
    '            self.chapter_switch_until = None;\n            self.launcher.refresh();\n',
    '            self.chapter_switch_until = None;\n            self.pomodoro_offer = false;\n            self.pomodoro_active = false;\n            self.pomodoro_break = false;\n            self.pomodoro_deadline = None;\n            self.command_capture = None;\n            self.launcher.refresh();\n',
    "pomodoro reset",
)
# Editor key prelude.
editor_prelude = '''    fn editor_key(&mut self, key: KeyEvent) {\n        if self.try_continue_to_next_chapter(key) {\n            return;\n        }\n\n'''
editor_new = r'''    fn editor_key(&mut self, key: KeyEvent) {
        if self.pomodoro_offer {
            match key.code {
                KeyCode::Enter | KeyCode::F(4) | KeyCode::Char('s') | KeyCode::Char('S')
                | KeyCode::Char('y') | KeyCode::Char('Y') => self.start_pomodoro(),
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.dismiss_pomodoro_offer()
                }
                _ => {}
            }
            return;
        }

        if key.code == KeyCode::F(4) && key.modifiers.is_empty() {
            self.start_pomodoro();
            return;
        }

        if self.capture_pd_command(key) {
            return;
        }

        if self.pomodoro_break {
            if key.modifiers.is_empty()
                && matches!(
                    key.code,
                    KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::PageUp
                        | KeyCode::PageDown
                        | KeyCode::Home
                        | KeyCode::End
                        | KeyCode::Esc
                )
                && let Some(editor) = self.editor.as_ref()
            {
                let _ = editor.send_key(key, editor.win32_input());
            }
            return;
        }

        if self.try_continue_to_next_chapter(key) {
            return;
        }

'''
s = replace_once(s, editor_prelude, editor_new, "editor key pomodoro prelude")
# Slint state updates: initial block and timer block.
initial_set = '''        ui.set_editor_active(model.editor.is_some());\n        ui.set_page_label(model.page_profile.paper.label().into());\n'''
initial_new = '''        ui.set_editor_active(model.editor.is_some());\n        ui.set_pomodoro_prompt(model.pomodoro_offer);\n        ui.set_pomodoro_break(model.pomodoro_break);\n        ui.set_pomodoro_text(model.pomodoro_label().into());\n        ui.set_page_label(model.page_profile.paper.label().into());\n'''
s = replace_once(s, initial_set, initial_new, "initial pomodoro UI")
# Replace second occurrence in timer by performing once more.
s = replace_once(s, initial_set, initial_new, "timer pomodoro UI")
# Key callback also refreshes prompt immediately.
key_ui = '''                ui.set_zen_active(model.zen_engaged());\n                ui.set_editor_active(model.editor.is_some());\n'''
key_ui_new = '''                ui.set_zen_active(model.zen_engaged());\n                ui.set_editor_active(model.editor.is_some());\n                ui.set_pomodoro_prompt(model.pomodoro_offer);\n                ui.set_pomodoro_break(model.pomodoro_break);\n                ui.set_pomodoro_text(model.pomodoro_label().into());\n'''
s = replace_once(s, key_ui, key_ui_new, "key callback pomodoro UI")
p.write_text(s, encoding="utf-8")

# ---------------- Cargo version ----------------
p = Path("Cargo.toml")
s = p.read_text(encoding="utf-8")
s = s.replace('version = "0.2.10"', 'version = "0.2.11"', 1)
p.write_text(s, encoding="utf-8")
