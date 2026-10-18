from pathlib import Path

p = Path('src/app.rs')
text = p.read_text(encoding='utf-8')

old = '''    fn close_current_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let index = self.active_tab.min(self.tabs.len() - 1);
        let closed = self.tabs.remove(index);
        self.closed_tabs.push(closed);
        if self.tabs.is_empty() {
            if let Some(editor) = self.editor.as_ref() {
                let _ = editor.send_command(":write");
                let _ = editor.send_command(":quit");
            }
            return;
        }
        let next = index.min(self.tabs.len() - 1);
        self.active_tab = usize::MAX;
        let _ = self.activate_tab(next);
    }
'''
new = '''    fn close_current_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        let index = self.active_tab.min(self.tabs.len() - 1);
        let closed = self.tabs.remove(index);
        self.closed_tabs.push(closed);
        if self.tabs.is_empty() {
            if let Some(editor) = self.editor.as_mut() {
                let _ = editor.send_command(":write");
                std::thread::sleep(Duration::from_millis(35));
                let _ = editor.flush_native();
                let _ = editor.send_command(":quit-all!");
            }
            self.zen_requested = false;
            self.editor = None;
            self.current_file = None;
            self.source_modified = None;
            self.page_profile = crate::page::PageProfile::default();
            self.chapter_switch_until = None;
            self.pomodoro_offer = false;
            self.pomodoro_active = false;
            self.pomodoro_break = false;
            self.pomodoro_deadline = None;
            self.command_capture = None;
            self.word_count = 0;
            self.active_tab = 0;
            self.f2_menu_open = false;
            self.f2_close_at = None;
            self.layout_redraw_at = None;
            self.layout_redraw_passes = 0;
            self.launcher.refresh();
            self.reset_parser();
            self.render_launcher();
            self.glyphs.clear();
            self.dirty = true;
            return;
        }
        let next = index.min(self.tabs.len() - 1);
        self.active_tab = usize::MAX;
        let _ = self.activate_tab(next);
    }
'''
if old not in text:
    raise SystemExit('close_current_tab block not found')
text = text.replace(old, new, 1)

old = '''        let ctrl_z = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('z') | KeyCode::Char('Z'));
        let ctrl_y = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y'));
        let ctrl_shift_z = ctrl_z && key.modifiers.contains(KeyModifiers::SHIFT);
        if ctrl_shift_z {
            if self.redo_style_change() {
                return;
            }
        } else if ctrl_z {
            if self.undo_style_change() {
                return;
            }
        } else if ctrl_y && self.redo_style_change() {
            return;
        }
'''
new = '''        let ctrl_z = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('z') | KeyCode::Char('Z'));
        let ctrl_y = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y'));
        let ctrl_shift_z = ctrl_z && key.modifiers.contains(KeyModifiers::SHIFT);
        if ctrl_shift_z || ctrl_y {
            if self.redo_style_change() {
                return;
            }
            if let Some(editor) = self.editor.as_ref() {
                let _ = editor.send_key(
                    KeyEvent::new(KeyCode::F(18), KeyModifiers::NONE),
                    editor.win32_input(),
                );
                self.layout_redraw_passes = 1;
                self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(55));
                self.dirty = true;
            }
            return;
        }
        if ctrl_z {
            if self.undo_style_change() {
                return;
            }
            if let Some(editor) = self.editor.as_ref() {
                let _ = editor.send_key(
                    KeyEvent::new(KeyCode::F(17), KeyModifiers::NONE),
                    editor.win32_input(),
                );
                self.layout_redraw_passes = 1;
                self.layout_redraw_at = Some(Instant::now() + Duration::from_millis(55));
                self.dirty = true;
            }
            return;
        }
'''
if old not in text:
    raise SystemExit('ctrl z block not found')
text = text.replace(old, new, 1)

start = text.index('fn visual_line_numbers(')
end = text.index('\nfn screen_line_number', start)
replacement = '''fn visual_line_numbers(
    screen: &vt100::Screen,
    document: &document::HsstDocument,
) -> (u16, Vec<(u16, usize)>) {
    let mut lines = Vec::new();
    for raw in document.body.split_inclusive('\\n') {
        let no_lf = raw.strip_suffix('\\n').unwrap_or(raw);
        lines.push(no_lf.strip_suffix('\\r').unwrap_or(no_lf));
    }
    if document.body.ends_with('\\n') {
        lines.push("");
    }
    if lines.is_empty() {
        lines.push("");
    }

    let (rows, cols) = screen.size();
    let mut gutter = 5u16;
    for row in 0..rows {
        if screen_line_number(screen, row, lines.len()).is_some() {
            let mut last_digit = None;
            for col in 0..cols.min(12) {
                if let Some(cell) = screen.cell(row, col) {
                    let content = cell.contents();
                    if content.chars().any(|ch| ch.is_ascii_digit()) {
                        last_digit = Some(col);
                    } else if last_digit.is_some() && !content.trim().is_empty() {
                        break;
                    }
                }
            }
            if let Some(last) = last_digit {
                gutter = (last + 2).max(4);
                break;
            }
        }
    }

    let text_width = cols.saturating_sub(gutter).max(1) as usize;
    let mut base = vec![1usize; lines.len()];
    let mut next = 1usize;
    for (index, line) in lines.iter().enumerate() {
        base[index] = next;
        next += prose_wrap_rows(line, text_width);
    }

    let mut out = Vec::new();
    let mut next_visible = None::<usize>;
    for row in 0..rows {
        if let Some(line_index) = screen_line_number(screen, row, lines.len()) {
            let number = base[line_index];
            out.push((row, number));
            next_visible = Some(number + 1);
            continue;
        }

        let Some(number) = next_visible else {
            continue;
        };

        let mut has_text = false;
        let mut is_editor_filler = false;
        for col in gutter..cols {
            let Some(cell) = screen.cell(row, col) else { continue; };
            if cell.is_wide_continuation() { continue; }
            let content = cell.contents();
            if !content.trim().is_empty() {
                if content.trim() == "~" {
                    is_editor_filler = true;
                }
                has_text = true;
                break;
            }
        }
        if is_editor_filler {
            next_visible = None;
            continue;
        }
        if has_text {
            out.push((row, number));
            next_visible = Some(number + 1);
        }
    }

    (gutter, out)
}
'''
text = text[:start] + replacement + text[end:]
p.write_text(text, encoding='utf-8')
