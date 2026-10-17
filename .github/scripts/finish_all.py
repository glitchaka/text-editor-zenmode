from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    text = p.read_text(encoding='utf-8')
    if old not in text:
        raise SystemExit(f'missing pattern in {path}: {old[:180]!r}')
    p.write_text(text.replace(old, new, 1), encoding='utf-8')

# --- WBW: restore explicit documentation/notes section in the complete workspace ---
replace_once('src/world.rs', '    References,\n    Review,\n', '    References,\n    Notes,\n    Review,\n')
replace_once('src/world.rs', '    pub const ALL: [Self; 16] = [\n', '    pub const ALL: [Self; 17] = [\n')
replace_once('src/world.rs', '        Self::References,\n        Self::Review,\n', '        Self::References,\n        Self::Notes,\n        Self::Review,\n')
replace_once('src/world.rs', '            Self::References => "Textos / referencias",\n            Self::Review => "Revisión",\n', '            Self::References => "Textos / referencias",\n            Self::Notes => "Documentación / notas",\n            Self::Review => "Revisión",\n')
replace_once('src/world.rs', '            Self::References => "references",\n            Self::Review => "review",\n', '            Self::References => "references",\n            Self::Notes => "notes",\n            Self::Review => "review",\n')
replace_once('src/world.rs', '                | Self::References\n                | Self::Review\n', '                | Self::References\n                | Self::Notes\n                | Self::Review\n')
replace_once('src/world.rs', '    pub references: usize,\n    pub review: usize,\n', '    pub references: usize,\n    pub notes: usize,\n    pub review: usize,\n')
replace_once('src/world.rs', '            WorldSection::References => self.references,\n            WorldSection::Review => self.review,\n', '            WorldSection::References => self.references,\n            WorldSection::Notes => self.notes,\n            WorldSection::Review => self.review,\n')
replace_once('src/world.rs', '        summary.references = self.array(WorldSection::References).len();\n        summary.review = self.array(WorldSection::Review).len();\n', '        summary.references = self.array(WorldSection::References).len();\n        summary.notes = self.array(WorldSection::Notes).len();\n        summary.review = self.array(WorldSection::Review).len();\n')
replace_once('src/world.rs', '                    stat_row("Referencias", summary.references),\n                    stat_row("Notas de revisión", summary.review),\n', '                    stat_row("Referencias", summary.references),\n                    stat_row("Documentos / notas", summary.notes),\n                    stat_row("Notas de revisión", summary.review),\n')
replace_once('src/world.rs', '''            WorldSection::References => json!({
                "id": generate_id(), "title": title, "type": "reference", "target": "", "source_name": "",
                "imported_at": unix_seconds(), "summary": "", "tags": [], "mentions": []
            }),
            WorldSection::Review => json!({
''', '''            WorldSection::References => json!({
                "id": generate_id(), "title": title, "type": "reference", "target": "", "source_name": "",
                "imported_at": unix_seconds(), "summary": "", "tags": [], "mentions": []
            }),
            WorldSection::Notes => json!({
                "id": generate_id(), "title": title, "body": "", "category": "documentation", "tags": [],
                "references": [], "attachments": [], "updated": unix_seconds()
            }),
            WorldSection::Review => json!({
''')
replace_once('src/world.rs', '            "timeline": [], "world": [], "maps": [], "magic": [], "references": [], "review": [], "media": []\n', '            "timeline": [], "world": [], "maps": [], "magic": [], "references": [], "notes": [], "review": [], "media": []\n')
replace_once('src/world.rs', '        assert_eq!(WorldSection::ALL.len(), 16);\n', '        assert_eq!(WorldSection::ALL.len(), 17);\n')

# --- Mouse selection: a simple click positions the caret and returns to insert mode; a drag remains a selection. ---
replace_once('src/app.rs', '        property <bool> terminal-selecting: false;\n', '        property <bool> terminal-selecting: false;\n        property <bool> terminal-select-dragged: false;\n')
replace_once('src/app.rs', '''                        root.terminal-selecting = root.editor-active
                            && root.terminal-cell-has-text(self.mouse-x / 1px, self.mouse-y / 1px);
                        if (root.terminal-selecting) {
''', '''                        root.terminal-selecting = root.editor-active
                            && root.terminal-cell-has-text(self.mouse-x / 1px, self.mouse-y / 1px);
                        root.terminal-select-dragged = false;
                        if (root.terminal-selecting) {
''')
replace_once('src/app.rs', '''                    } else if (event.button == PointerEventButton.left && event.kind == PointerEventKind.up && root.terminal-selecting) {
                        root.terminal-pointer(self.mouse-x / 1px, self.mouse-y / 1px, 1);
                        root.terminal-selecting = false;
                        EventResult.accept
''', '''                    } else if (event.button == PointerEventButton.left && event.kind == PointerEventKind.up && root.terminal-selecting) {
                        root.terminal-pointer(self.mouse-x / 1px, self.mouse-y / 1px, 1);
                        if (!root.terminal-select-dragged) {
                            root.terminal-pointer(self.mouse-x / 1px, self.mouse-y / 1px, 3);
                        }
                        root.terminal-selecting = false;
                        root.terminal-select-dragged = false;
                        EventResult.accept
''')
replace_once('src/app.rs', '''                    } else if (event.kind == PointerEventKind.cancel) {
                        root.terminal-selecting = false;
                        EventResult.reject
''', '''                    } else if (event.kind == PointerEventKind.cancel) {
                        root.terminal-selecting = false;
                        root.terminal-select-dragged = false;
                        EventResult.reject
''')
replace_once('src/app.rs', '''                    if (root.editor-active && root.terminal-selecting && self.pressed) {
                        root.terminal-pointer(self.mouse-x / 1px, self.mouse-y / 1px, 2);
                    }
''', '''                    if (root.editor-active && root.terminal-selecting && self.pressed) {
                        root.terminal-select-dragged = true;
                        root.terminal-pointer(self.mouse-x / 1px, self.mouse-y / 1px, 2);
                    }
''')
replace_once('src/app.rs', '''        let report = match kind {
            0 => format!("\\x1b[<0;{column};{line}M"),
            1 => format!("\\x1b[<0;{column};{line}m"),
            2 => format!("\\x1b[<32;{column};{line}M"),
            _ => return,
        };
        let _ = editor.write_reply(report.as_bytes());
        self.dirty = true;
''', '''        if kind == 3 {
            let win32 = editor.win32_input();
            let _ = editor.send_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), win32);
            let _ = editor.send_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), win32);
            self.dirty = true;
            return;
        }
        let report = match kind {
            0 => format!("\\x1b[<0;{column};{line}M"),
            1 => format!("\\x1b[<0;{column};{line}m"),
            2 => format!("\\x1b[<32;{column};{line}M"),
            _ => return,
        };
        let _ = editor.write_reply(report.as_bytes());
        self.dirty = true;
''')

# Close F2 sooner after the action has been accepted, while preserving the VT parser.
replace_once('src/app.rs', 'self.f2_close_at = Some(Instant::now() + Duration::from_millis(240));', 'self.f2_close_at = Some(Instant::now() + Duration::from_millis(120));')

# --- Scene board: render scene cards with literary metadata instead of a plain file-like list. ---
old = '''                        if rows.is_empty() {
                            push_line(&mut out, &framed_left("(sin registros)", inner_width, "38;5;244"));
                        } else {
                            for (index, row) in rows.iter().enumerate().take((rows.len()).min(14)) {
                                let marker = if index == self.launcher.workspace_row { "▶" } else { " " };
                                let detail = if row.detail.is_empty() { String::new() } else { format!(" · {}", row.detail) };
                                let line = format!("{marker} {}{}", row.label, detail);
                                push_line(&mut out, &framed_left(&truncate(&line, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                            }
                        }
'''
new = '''                        if rows.is_empty() {
                            push_line(&mut out, &framed_left("(sin registros)", inner_width, "38;5;244"));
                        } else if section == WorldSection::Scenes {
                            for (index, row) in rows.iter().enumerate().take((rows.len()).min(8)) {
                                let marker = if index == self.launcher.workspace_row { "▶" } else { " " };
                                let fields = workspace.record_fields(section, index);
                                let get = |key: &str| fields.iter().find(|field| field.label == key).map(|field| field.detail.as_str()).unwrap_or("");
                                let chapter = get("chapter");
                                let pov = get("pov");
                                let location = get("location");
                                let status = get("status");
                                let head = format!("{marker} ┌ {}  [cap. {} · {}]", row.label, if chapter.is_empty() { "—" } else { chapter }, status);
                                let meta = format!("  └ POV: {} · Lugar: {}", if pov.is_empty() { "—" } else { pov }, if location.is_empty() { "—" } else { location });
                                push_line(&mut out, &framed_left(&truncate(&head, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                                push_line(&mut out, &framed_left(&truncate(&meta, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                            }
                        } else {
                            for (index, row) in rows.iter().enumerate().take((rows.len()).min(14)) {
                                let marker = if index == self.launcher.workspace_row { "▶" } else { " " };
                                let detail = if row.detail.is_empty() { String::new() } else { format!(" · {}", row.detail) };
                                let line = format!("{marker} {}{}", row.label, detail);
                                push_line(&mut out, &framed_left(&truncate(&line, inner_width.saturating_sub(2)), inner_width, "38;5;244"));
                            }
                        }
'''
replace_once('src/app.rs', old, new)

# Documentation strings.
readme = Path('README.md')
text = readme.read_text(encoding='utf-8')
text = text.replace('referencias, revisión, maquetación, multimedia, configuración y estadísticas', 'referencias, documentación/notas, revisión, maquetación, multimedia, configuración y estadísticas')
readme.write_text(text, encoding='utf-8')

changelog = Path('CHANGELOG.md')
log = changelog.read_text(encoding='utf-8')
entry = '''## Completion pass — 2026-10-17\n\n- Añadida Documentación/notas como módulo WBW editable.\n- Tablero de escenas muestra tarjetas con capítulo, POV, ubicación y estado; mantiene creación, ficha, reordenación y edición.\n- Un clic de ratón posiciona el cursor y vuelve a escritura; arrastrar conserva selección, evitando el estado residual que provocaba inserciones/repeticiones inesperadas.\n- El cierre de F2 conserva el parser VT y vuelve a escritura después de aplicar el formato.\n- Se mantienen soft-wrap no destructivo, esquinas DWM nativas, arrastre de superficie completa y reflow sin reiniciar el parser.\n\n'''
if '## Completion pass — 2026-10-17' not in log:
    marker = log.find('## ')
    log = log[:marker] + entry + log[marker:] if marker >= 0 else entry + log
    changelog.write_text(log, encoding='utf-8')
