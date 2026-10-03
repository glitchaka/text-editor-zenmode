use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fontdue::{Font, FontSettings, Metrics};
use slint::{
    BackendSelector, ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, SharedString, Timer,
    TimerMode,
};

use crate::{
    editor::EditorSession,
    pty_protocol::Replies,
};

const INITIAL_COLS: u16 = 112;
const INITIAL_ROWS: u16 = 34;
const PAD_X: f32 = 14.0;
const PAD_Y: f32 = 12.0;
const CELL_WIDTH: f32 = 8.0;
const CELL_HEIGHT: f32 = 18.0;
const FONT_SIZE: f32 = 14.0;

const BG: Rgb = Rgb(0x11, 0x16, 0x19);
const FG: Rgb = Rgb(0xDF, 0xE8, 0xEF);
const CURSOR: Rgb = Rgb(0xE8, 0xCC, 0x83);

const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/JetBrainsMonoNerdFontMono-Regular.ttf"
));

slint::slint! {
    export component ZenWindow inherits Window {
        title: "Helix-SST Zenmode";
        preferred-width: 980px;
        preferred-height: 680px;
        min-width: 520px;
        min-height: 340px;
        background: #111619;

        in property <image> terminal-image;
        callback key-input(string, bool, bool, bool);

        Image {
            x: 0;
            y: 0;
            width: 100%;
            height: 100%;
            source: root.terminal-image;
            image-fit: fill;
        }

        focus := FocusScope {
            x: 0;
            y: 0;
            width: 100%;
            height: 100%;
            focus-on-click: true;
            focus-on-tab-navigation: false;

            init => {
                self.focus();
            }

            key-pressed(event) => {
                root.key-input(
                    event.text,
                    event.modifiers.control,
                    event.modifiers.alt,
                    event.modifiers.shift
                );
                accept
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Rgb(u8, u8, u8);

struct Glyph {
    metrics: Metrics,
    alpha: Vec<u8>,
}

#[derive(Clone)]
struct Entry {
    path: PathBuf,
    name: String,
    directory: bool,
}

struct Launcher {
    cwd: PathBuf,
    entries: Vec<Entry>,
    selected: usize,
    creating: bool,
    new_name: String,
    message: Option<String>,
}

impl Launcher {
    fn new(cwd: PathBuf) -> Self {
        let mut this = Self {
            cwd,
            entries: Vec::new(),
            selected: 0,
            creating: false,
            new_name: String::new(),
            message: None,
        };
        this.refresh();
        this
    }

    fn refresh(&mut self) {
        let mut entries = match fs::read_dir(&self.cwd) {
            Ok(read_dir) => read_dir
                .flatten()
                .map(|entry| {
                    let path = entry.path();
                    let directory = path.is_dir();
                    let name = entry.file_name().to_string_lossy().into_owned();
                    Entry {
                        path,
                        name,
                        directory,
                    }
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                self.message = Some(format!("No se pudo leer el directorio: {error}"));
                Vec::new()
            }
        };

        entries.sort_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        self.entries = entries;
        self.selected = self.selected.min(self.entries.len());
    }

    fn selected_entry(&self) -> Option<&Entry> {
        self.selected
            .checked_sub(1)
            .and_then(|index| self.entries.get(index))
    }
}

struct TerminalModel {
    parser: vt100::Parser<Replies>,
    font: Font,
    glyphs: HashMap<(char, u16), Glyph>,
    launcher: Launcher,
    editor: Option<EditorSession>,
    width: u32,
    height: u32,
    scale: f32,
    dirty: bool,
}

impl TerminalModel {
    fn new(initial: Option<PathBuf>) -> Result<Self> {
        let current = std::env::current_dir()?;
        let (cwd, file_to_open) = match initial {
            Some(path) if path.is_dir() => (path, None),
            Some(path) => {
                let absolute = if path.is_absolute() {
                    path
                } else {
                    current.join(path)
                };
                let parent = absolute
                    .parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .unwrap_or(&current)
                    .to_path_buf();
                (parent, Some(absolute))
            }
            None => (current, None),
        };

        let font = Font::from_bytes(FONT_BYTES, FontSettings::default())
            .map_err(|error| anyhow::anyhow!("No se pudo cargar la fuente: {error}"))?;

        let mut this = Self {
            parser: vt100::Parser::new_with_callbacks(
                INITIAL_ROWS,
                INITIAL_COLS,
                2_000,
                Replies::default(),
            ),
            font,
            glyphs: HashMap::new(),
            launcher: Launcher::new(cwd),
            editor: None,
            width: 980,
            height: 680,
            scale: 1.0,
            dirty: true,
        };

        if let Some(file) = file_to_open {
            if let Err(error) = this.open_editor(file) {
                this.launcher.message = Some(error.to_string());
                this.render_launcher();
            }
        } else {
            this.render_launcher();
        }

        Ok(this)
    }

    fn terminal_size(&self) -> (u16, u16) {
        let scale = self.scale.max(0.5);
        let cell_width = (CELL_WIDTH * scale).max(1.0);
        let cell_height = (CELL_HEIGHT * scale).max(1.0);
        let pad_x = PAD_X * scale;
        let pad_y = PAD_Y * scale;

        let cols = (((self.width as f32 - pad_x * 2.0) / cell_width).floor() as i32)
            .clamp(20, 300) as u16;
        let rows = (((self.height as f32 - pad_y * 2.0) / cell_height).floor() as i32)
            .clamp(8, 160) as u16;
        (cols, rows)
    }

    fn resize(&mut self, width: u32, height: u32, scale: f32) {
        let width = width.max(1);
        let height = height.max(1);
        let scale = scale.max(0.5);

        if self.width == width
            && self.height == height
            && (self.scale - scale).abs() < f32::EPSILON
        {
            return;
        }

        self.width = width;
        self.height = height;
        self.scale = scale;

        let (cols, rows) = self.terminal_size();
        if self.parser.screen().size() != (rows, cols) {
            self.parser.screen_mut().set_size(rows, cols);
            if let Some(editor) = self.editor.as_mut() {
                if let Err(error) = editor.resize(cols, rows) {
                    self.launcher.message = Some(format!("No se pudo redimensionar Helix: {error}"));
                }
            } else {
                self.render_launcher();
            }
        }

        self.glyphs.clear();
        self.dirty = true;
    }

    fn reset_parser(&mut self) {
        let (cols, rows) = self.terminal_size();
        self.parser = vt100::Parser::new_with_callbacks(rows, cols, 2_000, Replies::default());
    }

    fn open_editor(&mut self, file: PathBuf) -> Result<()> {
        let (cols, rows) = self.terminal_size();
        let session = EditorSession::start(&file, cols, rows)
            .with_context(|| format!("No se pudo abrir {}", file.display()))?;
        self.reset_parser();
        self.editor = Some(session);
        self.dirty = true;
        Ok(())
    }

    fn tick(&mut self) {
        let mut finished = false;

        if let Some(editor) = self.editor.as_mut() {
            while let Some(result) = editor.try_output() {
                match result {
                    Ok(bytes) => {
                        let bytes = editor.normalize_output(&bytes);
                        self.parser.process(&bytes);

                        let replies = std::mem::take(&mut self.parser.callbacks_mut().bytes);
                        if !replies.is_empty() {
                            let _ = editor.write_reply(&replies);
                        }

                        self.dirty = true;
                    }
                    Err(error) => {
                        self.launcher.message = Some(format!("Error leyendo Helix: {error}"));
                        finished = true;
                        break;
                    }
                }
            }

            if editor.exit_status().is_some() {
                finished = true;
            }
        }

        if finished {
            self.editor = None;
            self.launcher.refresh();
            self.reset_parser();
            self.render_launcher();
        }
    }

    fn key_event(&mut self, key: KeyEvent) {
        if self.editor.is_some() {
            self.editor_key(key);
        } else {
            self.launcher_key(key);
        }
    }

    fn editor_key(&mut self, key: KeyEvent) {
        let ctrl_v = key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V'));

        if ctrl_v {
            if let Ok(mut clipboard) = arboard::Clipboard::new()
                && let Ok(text) = clipboard.get_text()
                && let Some(editor) = self.editor.as_ref()
            {
                let _ = editor.paste(&text);
            }
            return;
        }

        let win32 = self.parser.callbacks().win32_input;
        if let Some(editor) = self.editor.as_ref() {
            let _ = editor.send_key(key, win32);
        }
    }

    fn launcher_key(&mut self, key: KeyEvent) {
        if self.launcher.creating {
            match key.code {
                KeyCode::Esc => {
                    self.launcher.creating = false;
                    self.launcher.new_name.clear();
                    self.launcher.message = None;
                }
                KeyCode::Enter => {
                    let name = self.launcher.new_name.trim().to_owned();
                    if name.is_empty() {
                        self.launcher.message = Some("Escribe un nombre de archivo.".into());
                    } else {
                        let path = self.launcher.cwd.join(&name);
                        if path.is_dir() {
                            self.launcher.message =
                                Some("Ese nombre corresponde a un directorio.".into());
                        } else {
                            match fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(&path)
                            {
                                Ok(_) => {
                                    self.launcher.creating = false;
                                    self.launcher.new_name.clear();
                                    self.launcher.message = None;
                                    if let Err(error) = self.open_editor(path) {
                                        self.launcher.message = Some(error.to_string());
                                    }
                                }
                                Err(error) => {
                                    self.launcher.message =
                                        Some(format!("No se pudo crear el archivo: {error}"));
                                }
                            }
                        }
                    }
                }
                KeyCode::Backspace => {
                    self.launcher.new_name.pop();
                }
                KeyCode::Char(ch)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !matches!(ch, '\r' | '\n') =>
                {
                    self.launcher.new_name.push(ch);
                }
                _ => {}
            }
            self.render_launcher();
            return;
        }

        match key.code {
            KeyCode::Up => {
                self.launcher.selected = self.launcher.selected.saturating_sub(1);
            }
            KeyCode::Down => {
                self.launcher.selected =
                    (self.launcher.selected + 1).min(self.launcher.entries.len());
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.launcher.creating = true;
                self.launcher.new_name.clear();
                self.launcher.message = None;
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.launcher.refresh();
            }
            KeyCode::Backspace => {
                if let Some(parent) = self.launcher.cwd.parent().map(Path::to_path_buf) {
                    self.launcher.cwd = parent;
                    self.launcher.selected = 0;
                    self.launcher.message = None;
                    self.launcher.refresh();
                }
            }
            KeyCode::Esc => {
                let _ = slint::quit_event_loop();
            }
            KeyCode::Enter => {
                if self.launcher.selected == 0 {
                    self.launcher.creating = true;
                    self.launcher.new_name.clear();
                    self.launcher.message = None;
                } else if let Some(entry) = self.launcher.selected_entry().cloned() {
                    if entry.directory {
                        self.launcher.cwd = entry.path;
                        self.launcher.selected = 0;
                        self.launcher.message = None;
                        self.launcher.refresh();
                    } else if let Err(error) = self.open_editor(entry.path) {
                        self.launcher.message = Some(error.to_string());
                    }
                }
            }
            _ => {}
        }

        if self.editor.is_none() {
            self.render_launcher();
        }
    }

    fn render_launcher(&mut self) {
        let (cols, rows) = self.terminal_size();
        let width = cols as usize;

        let mut out = String::from("\x1b[2J\x1b[H\x1b[?25l");
        push_line(&mut out, "\x1b[1;38;5;222mHELIX-SST\x1b[0m");
        push_line(
            &mut out,
            &format!(
                "\x1b[38;5;244m{}\x1b[0m",
                truncate(&self.launcher.cwd.display().to_string(), width.saturating_sub(2))
            ),
        );
        push_line(&mut out, "");
        push_line(
            &mut out,
            &"─".repeat(width.saturating_sub(2).min(80)),
        );

        if self.launcher.creating {
            push_line(&mut out, "");
            push_line(&mut out, "\x1b[1mNuevo archivo\x1b[0m");
            push_line(
                &mut out,
                &format!("Nombre: {}\x1b[?25h", self.launcher.new_name),
            );
            push_line(&mut out, "");
            push_line(&mut out, "\x1b[38;5;244mEnter crear · Esc cancelar\x1b[0m");
        } else {
            let available = rows.saturating_sub(9) as usize;
            let selected = self.launcher.selected;
            let total = self.launcher.entries.len() + 1;
            let start = selected.saturating_sub(available.saturating_sub(1) / 2);
            let end = (start + available).min(total);

            for index in start..end {
                if index == 0 {
                    let marker = if selected == 0 { ">" } else { " " };
                    let style = if selected == 0 { "\x1b[1;38;5;222m" } else { "" };
                    push_line(
                        &mut out,
                        &format!("{style}{marker} [ Nuevo archivo ]\x1b[0m"),
                    );
                    continue;
                }

                if let Some(entry) = self.launcher.entries.get(index - 1) {
                    let marker = if selected == index { ">" } else { " " };
                    let suffix = if entry.directory { "/" } else { "" };
                    let icon = if entry.directory { "▸" } else { " " };
                    let label = truncate(
                        &format!("{icon} {}{suffix}", entry.name),
                        width.saturating_sub(4),
                    );
                    let style = if selected == index {
                        "\x1b[1;38;5;117m"
                    } else if entry.directory {
                        "\x1b[38;5;109m"
                    } else {
                        ""
                    };
                    push_line(&mut out, &format!("{style}{marker} {label}\x1b[0m"));
                }
            }

            push_line(&mut out, "");
            push_line(
                &mut out,
                "\x1b[38;5;244m↑/↓ mover · Enter abrir · N nuevo · Backspace subir · R refrescar\x1b[0m",
            );
        }

        if let Some(message) = self.launcher.message.as_deref() {
            push_line(&mut out, "");
            push_line(
                &mut out,
                &format!(
                    "\x1b[38;5;203m{}\x1b[0m",
                    truncate(message, width.saturating_sub(2))
                ),
            );
        }

        self.parser.process(out.as_bytes());
        self.parser.callbacks_mut().bytes.clear();
        self.dirty = true;
    }

    fn render(&mut self) -> Image {
        let width = self.width.max(1);
        let height = self.height.max(1);
        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        let pixels = buffer.make_mut_slice();
        pixels.fill(Rgba8Pixel {
            r: BG.0,
            g: BG.1,
            b: BG.2,
            a: 255,
        });

        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        let cursor = screen.cursor_position();
        let cursor_on = !screen.hide_cursor();

        let scale = self.scale.max(0.5);
        let left_pad = (PAD_X * scale).round();
        let top_pad = (PAD_Y * scale).round();
        let cell_width = (CELL_WIDTH * scale).round().max(1.0);
        let cell_height = (CELL_HEIGHT * scale).round().max(1.0);
        let font_px = (FONT_SIZE * scale).round().max(8.0);
        let font_key = font_px.round() as u16;

        for row in 0..rows {
            for col in 0..cols {
                let Some(cell) = screen.cell(row, col) else {
                    continue;
                };
                if cell.is_wide_continuation() {
                    continue;
                }

                let mut fg = terminal_color(cell.fgcolor(), FG);
                let mut bg = terminal_color(cell.bgcolor(), BG);
                let mut paint_background =
                    !matches!(cell.bgcolor(), vt100::Color::Default) || cell.inverse();

                if cell.inverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }

                if cursor_on && cursor == (row, col) {
                    fg = BG;
                    bg = CURSOR;
                    paint_background = true;
                }

                let x = (left_pad + col as f32 * cell_width).round() as i32;
                let y = (top_pad + row as f32 * cell_height).round() as i32;
                let wide = if cell.is_wide() { 2.0 } else { 1.0 };
                let w = (cell_width * wide).ceil() as i32;
                let h = cell_height.ceil() as i32;

                if paint_background {
                    fill_rect(pixels, width, height, x, y, w, h, bg);
                }

                let content = cell.contents();
                if content.is_empty() {
                    continue;
                }

                let mut pen_x = x;
                let baseline = y + (cell_height * 0.80).round() as i32;
                for ch in content.chars() {
                    let key = (ch, font_key);
                    if !self.glyphs.contains_key(&key) {
                        let (metrics, alpha) = self.font.rasterize(ch, font_px);
                        self.glyphs.insert(key, Glyph { metrics, alpha });
                    }

                    if let Some(glyph) = self.glyphs.get(&key) {
                        draw_glyph(pixels, width, height, pen_x, baseline, glyph, fg);
                        if cell.bold() {
                            draw_glyph(
                                pixels,
                                width,
                                height,
                                pen_x + scale.max(1.0).round() as i32,
                                baseline,
                                glyph,
                                fg,
                            );
                        }
                        pen_x += glyph.metrics.advance_width.round() as i32;
                    }
                }
            }
        }

        self.dirty = false;
        Image::from_rgba8_premultiplied(buffer)
    }
}

pub fn run(initial: Option<PathBuf>) -> Result<()> {
    BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()
        .map_err(|error| anyhow::anyhow!("No se pudo inicializar Winit/FemtoVG: {error}"))?;

    let model = std::rc::Rc::new(std::cell::RefCell::new(TerminalModel::new(initial)?));
    let ui = ZenWindow::new()?;

    {
        let model = model.clone();
        ui.on_key_input(move |text, ctrl, alt, shift| {
            handle_key(&mut model.borrow_mut(), text.as_str(), ctrl, alt, shift);
        });
    }

    ui.show()?;

    let weak = ui.as_weak();
    let timer = Timer::default();
    {
        let model = model.clone();
        timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };

            let size = ui.window().size();
            let scale = ui.window().scale_factor();

            let mut model = model.borrow_mut();
            model.resize(size.width, size.height, scale);
            model.tick();

            if model.dirty {
                ui.set_terminal_image(model.render());
            }
        });
    }

    slint::run_event_loop()?;
    timer.stop();
    Ok(())
}

fn handle_key(
    model: &mut TerminalModel,
    text: &str,
    ctrl: bool,
    alt: bool,
    shift: bool,
) {
    use slint::platform::Key;

    if [
        Key::Shift,
        Key::ShiftR,
        Key::Control,
        Key::ControlR,
        Key::Alt,
        Key::AltGr,
        Key::Meta,
        Key::MetaR,
        Key::CapsLock,
    ]
    .into_iter()
    .any(|key| key_is(text, key))
    {
        return;
    }

    let mut modifiers = KeyModifiers::NONE;
    if ctrl {
        modifiers |= KeyModifiers::CONTROL;
    }
    if alt {
        modifiers |= KeyModifiers::ALT;
    }
    if shift {
        modifiers |= KeyModifiers::SHIFT;
    }

    if let Some(ch) = text.chars().next()
        && text.chars().count() == 1
        && ('\x01'..='\x1a').contains(&ch)
    {
        modifiers |= KeyModifiers::CONTROL;
        let letter = (ch as u8 + b'a' - 1) as char;
        model.key_event(KeyEvent::new(KeyCode::Char(letter), modifiers));
        return;
    }

    if let Some(code) = raw_key_code(text, shift) {
        model.key_event(KeyEvent::new(code, modifiers));
    }
}

fn key_is(text: &str, key: slint::platform::Key) -> bool {
    let encoded: SharedString = key.into();
    text == encoded.as_str()
}

fn raw_key_code(text: &str, shift: bool) -> Option<KeyCode> {
    use slint::platform::Key;

    let special = [
        (Key::Escape, KeyCode::Esc),
        (Key::Return, KeyCode::Enter),
        (Key::Backspace, KeyCode::Backspace),
        (Key::UpArrow, KeyCode::Up),
        (Key::DownArrow, KeyCode::Down),
        (Key::LeftArrow, KeyCode::Left),
        (Key::RightArrow, KeyCode::Right),
        (Key::Home, KeyCode::Home),
        (Key::End, KeyCode::End),
        (Key::Delete, KeyCode::Delete),
        (Key::Insert, KeyCode::Insert),
        (Key::PageUp, KeyCode::PageUp),
        (Key::PageDown, KeyCode::PageDown),
        (Key::F1, KeyCode::F(1)),
        (Key::F2, KeyCode::F(2)),
        (Key::F3, KeyCode::F(3)),
        (Key::F4, KeyCode::F(4)),
        (Key::F5, KeyCode::F(5)),
        (Key::F6, KeyCode::F(6)),
        (Key::F7, KeyCode::F(7)),
        (Key::F8, KeyCode::F(8)),
        (Key::F9, KeyCode::F(9)),
        (Key::F10, KeyCode::F(10)),
        (Key::F11, KeyCode::F(11)),
        (Key::F12, KeyCode::F(12)),
    ];

    if key_is(text, Key::Tab) {
        return Some(if shift {
            KeyCode::BackTab
        } else {
            KeyCode::Tab
        });
    }
    if key_is(text, Key::Backtab) {
        return Some(KeyCode::BackTab);
    }

    for (key, code) in special {
        if key_is(text, key) {
            return Some(code);
        }
    }

    let mut chars = text.chars();
    let ch = chars.next()?;
    (chars.next().is_none() && !ch.is_control()).then_some(KeyCode::Char(ch))
}

fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push_str("\r\n");
}

fn truncate(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut chars = text.chars();
    let taken = chars.by_ref().take(max).collect::<String>();
    if chars.next().is_some() && max > 1 {
        let mut shortened = taken.chars().take(max - 1).collect::<String>();
        shortened.push('…');
        shortened
    } else {
        taken
    }
}

fn terminal_color(value: vt100::Color, default: Rgb) -> Rgb {
    const COLORS: [Rgb; 16] = [
        Rgb(0x11, 0x16, 0x19),
        Rgb(0xF2, 0x6B, 0x6B),
        Rgb(0xA3, 0xC7, 0x86),
        Rgb(0xE8, 0xCC, 0x83),
        Rgb(0x82, 0xAD, 0xE0),
        Rgb(0xC9, 0x9F, 0xCE),
        Rgb(0xC0, 0xCD, 0xD7),
        Rgb(0xDF, 0xE8, 0xEF),
        Rgb(0x67, 0x6E, 0x75),
        Rgb(0xFF, 0x87, 0x87),
        Rgb(0xC4, 0xEB, 0xA8),
        Rgb(0xFF, 0xE8, 0xA6),
        Rgb(0xA8, 0xD1, 0xFF),
        Rgb(0xEB, 0xC1, 0xF0),
        Rgb(0xE2, 0xEF, 0xF9),
        Rgb(0xFF, 0xFF, 0xFF),
    ];

    match value {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => Rgb(r, g, b),
        vt100::Color::Idx(i) if i < 16 => COLORS[i as usize],
        vt100::Color::Idx(i) if i >= 232 => {
            let v = 8 + (i - 232) * 10;
            Rgb(v, v, v)
        }
        vt100::Color::Idx(i) => {
            let i = i - 16;
            let component = |n| if n == 0 { 0 } else { 55 + n * 40 };
            Rgb(component(i / 36), component(i / 6 % 6), component(i % 6))
        }
    }
}

fn fill_rect(
    pixels: &mut [Rgba8Pixel],
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: Rgb,
) {
    let left = x.max(0) as u32;
    let top = y.max(0) as u32;
    let right = (x + w).max(0).min(width as i32) as u32;
    let bottom = (y + h).max(0).min(height as i32) as u32;

    for py in top..bottom {
        let start = (py * width + left) as usize;
        let end = (py * width + right) as usize;
        for pixel in &mut pixels[start..end] {
            *pixel = Rgba8Pixel {
                r: color.0,
                g: color.1,
                b: color.2,
                a: 255,
            };
        }
    }
}

fn draw_glyph(
    pixels: &mut [Rgba8Pixel],
    width: u32,
    height: u32,
    cell_x: i32,
    baseline: i32,
    glyph: &Glyph,
    color: Rgb,
) {
    if glyph.metrics.width == 0 || glyph.metrics.height == 0 {
        return;
    }

    let start_x = cell_x + glyph.metrics.xmin;
    let start_y = baseline - glyph.metrics.ymin - glyph.metrics.height as i32;

    for gy in 0..glyph.metrics.height {
        for gx in 0..glyph.metrics.width {
            let x = start_x + gx as i32;
            let y = start_y + gy as i32;
            if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                continue;
            }

            let alpha = glyph.alpha[gy * glyph.metrics.width + gx] as u16;
            if alpha == 0 {
                continue;
            }

            let index = y as usize * width as usize + x as usize;
            let dst = pixels[index];
            let inv = 255u16 - alpha;

            let sr = color.0 as u16 * alpha / 255;
            let sg = color.1 as u16 * alpha / 255;
            let sb = color.2 as u16 * alpha / 255;

            pixels[index] = Rgba8Pixel {
                r: (sr + dst.r as u16 * inv / 255).min(255) as u8,
                g: (sg + dst.g as u16 * inv / 255).min(255) as u8,
                b: (sb + dst.b as u16 * inv / 255).min(255) as u8,
                a: 255,
            };
        }
    }
}
