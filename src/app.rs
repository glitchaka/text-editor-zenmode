use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fontdue::{Font, FontSettings, Metrics};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{
    BackendSelector, ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, SharedString, Timer,
    TimerMode,
};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::HWND,
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::{
        Input::KeyboardAndMouse::{SetActiveWindow, SetFocus},
        WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, HWND_NOTOPMOST,
            HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetForegroundWindow,
            SetWindowPos,
        },
    },
};

use crate::editor::EditorSession;

const INITIAL_COLS: u16 = 112;
const INITIAL_ROWS: u16 = 34;
const PAD_X: f32 = 14.0;
const PAD_Y: f32 = 12.0;
const CELL_WIDTH: f32 = 8.0;
const CELL_HEIGHT: f32 = 18.0;
const FONT_SIZE: f32 = 14.0;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const ISLAND_TOP: f32 = 6.0;
const ISLAND_HEIGHT: f32 = 34.0;
const CONTENT_TOP_GAP: f32 = 8.0;
const SPLASH_DURATION: Duration = Duration::from_millis(1300);

// Retrofuturistic semigraphic icons for the TUI launcher.
// These private-use cells are intercepted by our terminal renderer and painted
// as 16×16 pixel panels spanning two character cells. They are not font icons.
const ICON_FOLDER: char = '\u{e100}';
const ICON_DOCUMENT: char = '\u{e101}';
const ICON_CODE: char = '\u{e102}';
const ICON_EXECUTABLE: char = '\u{e103}';
const ICON_COMPONENT: char = '\u{e104}';
const ICON_BUILD: char = '\u{e105}';
const ICON_IMAGE: char = '\u{e106}';
const ICON_AUDIO: char = '\u{e107}';
const ICON_VIDEO: char = '\u{e108}';
const ICON_ARCHIVE: char = '\u{e109}';
const ICON_PDF: char = '\u{e10a}';
const ICON_OFFICE: char = '\u{e10b}';
const ICON_LOCK: char = '\u{e10c}';
const ICON_RUST: char = '\u{e10d}';

const BG: Rgb = Rgb(0x11, 0x16, 0x19);
const FG: Rgb = Rgb(0xDF, 0xE8, 0xEF);
const CURSOR: Rgb = Rgb(0xE8, 0xCC, 0x83);

const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/JetBrainsMonoNerdFontMono-Regular.ttf"
));

slint::slint! {
    export component ZenWindow inherits Window {
        title: "Helix SST";
        preferred-width: 980px;
        preferred-height: 680px;
        min-width: 520px;
        min-height: 340px;
        no-frame: true;
        resize-border-width: 7px;
        background: #111619;

        in property <image> terminal-image;
        in property <string> version-text: "v0.0.0";
        in property <bool> zen-active: false;
        in property <bool> editor-active: false;
        callback key-input(string, bool, bool, bool);
        callback toggle-zen();
        callback close-window();

        Image {
            x: 0;
            y: 0;
            width: 100%;
            height: 100%;
            source: root.terminal-image;
            image-fit: fill;
        }

        terminal-focus := FocusScope {
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

        zen-reveal := TouchArea {
            x: 0;
            y: 0;
            width: 100%;
            height: 48px;
            enabled: root.zen-active;
        }

        island := Rectangle {
            visible: !root.zen-active
                || zen-reveal.has-hover
                || zen-title-hover.has-hover
                || zen-touch.has-hover
                || minimize-touch.has-hover
                || maximize-touch.has-hover
                || close-touch.has-hover;
            width: min(650px, root.width - 20px);
            height: 34px;
            x: (root.width - self.width) / 2;
            y: 6px;
            border-radius: 13px;
            background: rgba(10, 13, 20, 0.96);
            border-width: 1px;
            border-color: #2b3547;

            zen-title-hover := TouchArea {
                x: 0;
                y: 0;
                width: island.width - 114px;
                height: island.height;
                enabled: root.zen-active;
            }

            island-move := WindowMoveArea {
                x: 0;
                y: 0;
                width: parent.width;
                height: parent.height;

                Text {
                    x: 17px;
                    y: 0;
                    width: 120px;
                    height: parent.height;
                    text: "HELIX SST";
                    color: #dfe8ef;
                    font-family: "Segoe UI Variable";
                    font-size: 14px;
                    font-weight: 700;
                    vertical-alignment: center;
                }

                Text {
                    x: 137px;
                    y: 0;
                    width: 125px;
                    height: parent.height;
                    text: root.version-text;
                    color: #7f8b9b;
                    font-family: "Segoe UI Variable";
                    font-size: 12px;
                    vertical-alignment: center;
                }

                zen-control := Rectangle {
                    visible: island.width >= 470px;
                    x: (island.width - 104px) / 2;
                    y: 3px;
                    width: 104px;
                    height: island.height - 6px;
                    border-radius: 9px;
                    background: zen-touch.pressed
                        ? rgb(36, 49, 67)
                        : zen-touch.has-hover ? rgb(23, 35, 52) : transparent;

                    Text {
                        width: 100%;
                        height: 100%;
                        text: root.editor-active
                            ? (root.zen-active ? "ZENMODE ACTIVO" : "ENTRAR ZEN")
                            : "EDITOR";
                        color: root.zen-active ? #e8cc83 : #8db9bb;
                        font-family: "Segoe UI Variable";
                        font-size: 11px;
                        font-weight: 600;
                        vertical-alignment: center;
                        horizontal-alignment: center;
                    }

                    zen-touch := TouchArea {
                        enabled: root.editor-active;
                        mouse-cursor: pointer;
                        clicked => { root.toggle-zen(); }
                    }
                }

                Rectangle {
                    x: island.width - 114px;
                    y: 1px;
                    width: 38px;
                    height: island.height - 2px;
                    border-radius: 10px;
                    background: minimize-touch.pressed
                        ? rgb(36, 49, 67)
                        : minimize-touch.has-hover ? rgb(23, 35, 52) : transparent;

                    Path {
                        x: 11px;
                        y: 10px;
                        width: 16px;
                        height: 12px;
                        commands: "M 1 1 L 8 9 L 15 1";
                        stroke: #74c8f5;
                        stroke-width: 2.4px;
                        stroke-line-cap: round;
                        stroke-line-join: round;
                    }

                    minimize-touch := TouchArea {
                        mouse-cursor: pointer;
                        clicked => { root.minimized = true; }
                    }
                }

                Rectangle {
                    x: island.width - 76px;
                    y: 1px;
                    width: 38px;
                    height: island.height - 2px;
                    border-radius: 10px;
                    background: maximize-touch.pressed
                        ? rgb(36, 49, 67)
                        : maximize-touch.has-hover ? rgb(23, 35, 52) : transparent;

                    Path {
                        x: 11px;
                        y: 11px;
                        width: 16px;
                        height: 12px;
                        commands: "M 1 10 L 8 2 L 15 10";
                        stroke: #74c8f5;
                        stroke-width: 2.4px;
                        stroke-line-cap: round;
                        stroke-line-join: round;
                    }

                    maximize-touch := TouchArea {
                        mouse-cursor: pointer;
                        clicked => { root.maximized = !root.maximized; }
                    }
                }

                Rectangle {
                    x: island.width - 38px;
                    y: 1px;
                    width: 38px;
                    height: island.height - 2px;
                    border-radius: 10px;
                    background: close-touch.pressed
                        ? rgb(62, 23, 36)
                        : close-touch.has-hover ? rgb(48, 18, 28) : transparent;

                    Path {
                        x: 10px;
                        y: 8px;
                        width: 18px;
                        height: 18px;
                        commands: "M 9 1 L 9 8 M 3.3 3.7 A 7 7 0 1 0 14.7 3.7";
                        stroke: close-touch.has-hover ? #ff5d78 : #ff9fbd;
                        stroke-width: 2px;
                        stroke-line-cap: round;
                    }

                    close-touch := TouchArea {
                        mouse-cursor: pointer;
                        clicked => { root.close-window(); }
                    }
                }
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
        self.selected = self.selected.min(self.entries.len() + 2);
    }

    fn selected_entry(&self) -> Option<&Entry> {
        self.selected
            .checked_sub(3)
            .and_then(|index| self.entries.get(index))
    }
}

struct TerminalModel {
    parser: vt100::Parser,
    font: Font,
    glyphs: HashMap<(char, u16), Glyph>,
    launcher: Launcher,
    editor: Option<EditorSession>,
    width: u32,
    height: u32,
    scale: f32,
    dirty: bool,
    splash_active: bool,
    splash_started: Instant,
    pending_initial: Option<PathBuf>,
    zen_requested: bool,
}

impl TerminalModel {
    fn new(initial: Option<PathBuf>, zen_requested: bool) -> Result<Self> {
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
            parser: vt100::Parser::new(INITIAL_ROWS, INITIAL_COLS, 2_000),
            font,
            glyphs: HashMap::new(),
            launcher: Launcher::new(cwd),
            editor: None,
            width: 980,
            height: 680,
            scale: 1.0,
            dirty: true,
            splash_active: true,
            splash_started: Instant::now(),
            pending_initial: file_to_open,
            zen_requested,
        };

        this.render_splash();
        Ok(this)
    }

    fn zen_engaged(&self) -> bool {
        self.zen_requested && self.editor.is_some()
    }

    fn toggle_editor_zen(&mut self) {
        if self.editor.is_some() {
            self.zen_requested = !self.zen_requested;
            self.dirty = true;
        }
    }

    fn geometry(&self) -> (f32, f32, f32, f32) {
        let scale = self.scale.max(0.5);
        let left_pad = (PAD_X * scale).round();
        let top_pad = if self.zen_engaged() {
            (PAD_Y * scale).round()
        } else {
            ((ISLAND_TOP + ISLAND_HEIGHT + CONTENT_TOP_GAP + PAD_Y) * scale).round()
        };
        let cell_width = (CELL_WIDTH * scale).round().max(1.0);
        let cell_height = (CELL_HEIGHT * scale).round().max(1.0);
        (left_pad, top_pad, cell_width, cell_height)
    }

    fn terminal_size(&self) -> (u16, u16) {
        let (left_pad, top_pad, cell_width, cell_height) = self.geometry();
        let cols = (((self.width as f32 - left_pad * 2.0) / cell_width).floor() as i32)
            .clamp(20, 300) as u16;
        let bottom_pad = (PAD_Y * self.scale.max(0.5)).round();
        let rows = (((self.height as f32 - top_pad - bottom_pad) / cell_height).floor() as i32)
            .clamp(8, 160) as u16;
        (cols, rows)
    }

    fn resize(&mut self, width: u32, height: u32, scale: f32) {
        let width = width.max(1);
        let height = height.max(1);
        let scale = scale.max(0.5);

        if self.width == width && self.height == height && (self.scale - scale).abs() < f32::EPSILON
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
                    self.launcher.message =
                        Some(format!("No se pudo redimensionar Helix: {error}"));
                }
            } else if self.splash_active {
                self.render_splash();
            } else {
                self.render_launcher();
            }
        }

        self.glyphs.clear();
        self.dirty = true;
    }

    fn render_splash(&mut self) {
        let (cols, rows) = self.terminal_size();
        let width = cols as usize;
        let frame_width = width.saturating_sub(12).clamp(42, 72);
        let inner_width = frame_width.saturating_sub(2);
        let top_blank = (rows as usize).saturating_sub(11) / 3;

        let mut out = String::from("\x1b[2J\x1b[H\x1b[?25l");
        for _ in 0..top_blank {
            push_line(&mut out, "");
        }

        push_line(
            &mut out,
            &format!(
                "{}╭{}╮",
                " ".repeat((width.saturating_sub(frame_width)) / 2),
                "─".repeat(inner_width)
            ),
        );
        push_line(
            &mut out,
            &centered_frame_line(
                "\x1b[1;38;5;222m██  HELIX SST  ██\x1b[0m",
                width,
                frame_width,
            ),
        );
        push_line(
            &mut out,
            &centered_frame_line(
                &format!("\x1b[38;5;109mTERMINAL EDITOR SYSTEM · v{APP_VERSION}\x1b[0m"),
                width,
                frame_width,
            ),
        );
        push_line(&mut out, &centered_frame_line("", width, frame_width));
        push_line(
            &mut out,
            &centered_frame_line(
                "\x1b[38;5;250mWRITE  ·  EDIT  ·  FOCUS\x1b[0m",
                width,
                frame_width,
            ),
        );
        push_line(
            &mut out,
            &centered_frame_line(
                "\x1b[38;5;244msemigraphic console subsystem\x1b[0m",
                width,
                frame_width,
            ),
        );
        push_line(&mut out, &centered_frame_line("", width, frame_width));
        push_line(
            &mut out,
            &centered_frame_line(
                "\x1b[38;5;244mpresiona cualquier tecla para continuar\x1b[0m",
                width,
                frame_width,
            ),
        );
        push_last_line(
            &mut out,
            &format!(
                "{}╰{}╯",
                " ".repeat((width.saturating_sub(frame_width)) / 2),
                "─".repeat(inner_width)
            ),
        );

        self.parser.process(out.as_bytes());
        self.dirty = true;
    }

    fn finish_splash(&mut self) {
        if !self.splash_active {
            return;
        }

        self.splash_active = false;
        self.reset_parser();

        if let Some(file) = self.pending_initial.take() {
            if let Err(error) = self.open_editor(file) {
                self.launcher.message = Some(error.to_string());
                self.render_launcher();
            }
        } else {
            self.render_launcher();
        }
    }

    fn reset_parser(&mut self) {
        let (cols, rows) = self.terminal_size();
        self.parser = vt100::Parser::new(rows, cols, 2_000);
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
        if self.splash_active {
            if self.splash_started.elapsed() >= SPLASH_DURATION {
                self.finish_splash();
            }
            return;
        }

        let mut finished = false;

        if let Some(editor) = self.editor.as_mut() {
            let _ = editor.settle_startup_language();

            while let Some(result) = editor.try_output() {
                match result {
                    Ok(bytes) => {
                        let bytes = editor.normalize_output(&bytes);
                        self.parser.process(&bytes);
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
        if self.splash_active {
            self.finish_splash();
            return;
        }

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

        if let Some(editor) = self.editor.as_ref() {
            let _ = editor.send_key(key, editor.win32_input());
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
                            match fs::OpenOptions::new().create(true).append(true).open(&path) {
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
                    (self.launcher.selected + 1).min(self.launcher.entries.len() + 2);
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.launcher.selected = 2;
                self.launcher.creating = true;
                self.launcher.new_name.clear();
                self.launcher.message = None;
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.launcher.refresh();
            }
            KeyCode::Char('z') | KeyCode::Char('Z') => {
                self.zen_requested = !self.zen_requested;
                self.launcher.message = Some(if self.zen_requested {
                    "Zenmode real seleccionado.".into()
                } else {
                    "Modo normal seleccionado.".into()
                });
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
            KeyCode::Enter => match self.launcher.selected {
                0 => {
                    self.zen_requested = false;
                    self.launcher.message = Some("Modo normal seleccionado.".into());
                }
                1 => {
                    self.zen_requested = true;
                    self.launcher.message = Some("Zenmode real seleccionado.".into());
                }
                2 => {
                    self.launcher.creating = true;
                    self.launcher.new_name.clear();
                    self.launcher.message = None;
                }
                _ => {
                    if let Some(entry) = self.launcher.selected_entry().cloned() {
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
            },
            _ => {}
        }

        if self.editor.is_none() {
            self.render_launcher();
        }
    }

    fn render_launcher(&mut self) {
        let (cols, rows) = self.terminal_size();
        let width = cols as usize;
        let frame_width = width.saturating_sub(4).clamp(44, 96);
        let inner_width = frame_width.saturating_sub(2);

        let mut out = String::from("\x1b[2J\x1b[H\x1b[?25l");

        push_line(
            &mut out,
            &format!("\x1b[38;5;244m╭{}╮\x1b[0m", "─".repeat(inner_width)),
        );
        push_line(
            &mut out,
            &framed_center(
                &format!(
                    "\x1b[1;38;5;222mHELIX SST\x1b[0m  \x1b[38;5;250mversión {APP_VERSION}\x1b[0m"
                ),
                inner_width,
                "38;5;244",
            ),
        );
        push_line(
            &mut out,
            &framed_center(
                "\x1b[38;5;109meditor de texto · terminal zen\x1b[0m",
                inner_width,
                "38;5;244",
            ),
        );
        push_line(
            &mut out,
            &format!("\x1b[38;5;244m├{}┤\x1b[0m", "─".repeat(inner_width)),
        );

        let path_text = truncate(
            &self.launcher.cwd.display().to_string(),
            inner_width.saturating_sub(13),
        );
        push_line(
            &mut out,
            &framed_left(
                &format!("\x1b[38;5;244mDIRECTORIO\x1b[0m  \x1b[38;5;250m{path_text}\x1b[0m"),
                inner_width,
                "38;5;244",
            ),
        );

        push_line(
            &mut out,
            &format!("\x1b[38;5;244m├{}┤\x1b[0m", "─".repeat(inner_width)),
        );
        push_line(
            &mut out,
            &framed_left(
                "\x1b[1;38;5;222mMODO DE APERTURA\x1b[0m",
                inner_width,
                "38;5;244",
            ),
        );

        for (index, (label, active)) in [
            ("Normal", !self.zen_requested),
            ("Zenmode real", self.zen_requested),
        ]
        .into_iter()
        .enumerate()
        {
            let selected_now = self.launcher.selected == index;
            let marker = if selected_now { "▶" } else { " " };
            let state = if active {
                "\x1b[1;38;5;222m●\x1b[0m"
            } else {
                "\x1b[38;5;244m○\x1b[0m"
            };
            let row = if selected_now {
                format!("\x1b[1;38;5;117m{marker}\x1b[0m  {state}  \x1b[1;38;5;255m{label}\x1b[0m")
            } else {
                format!("{marker}  {state}  \x1b[38;5;250m{label}\x1b[0m")
            };
            push_line(&mut out, &framed_left(&row, inner_width, "38;5;244"));
        }

        push_line(
            &mut out,
            &format!("\x1b[38;5;244m├{}┤\x1b[0m", "─".repeat(inner_width)),
        );
        push_line(
            &mut out,
            &framed_left("\x1b[1;38;5;222mARCHIVOS\x1b[0m", inner_width, "38;5;244"),
        );

        if self.launcher.creating {
            push_line(&mut out, &framed_left("", inner_width, "38;5;244"));
            push_line(
                &mut out,
                &framed_left(
                    &format!("\x1b[1;38;5;222m{ICON_DOCUMENT}\x1b[0m   NUEVO ARCHIVO"),
                    inner_width,
                    "38;5;244",
                ),
            );
            push_line(
                &mut out,
                &framed_left(
                    &format!(
                        "\x1b[38;5;250mNombre:\x1b[0m {}\x1b[?25h",
                        self.launcher.new_name
                    ),
                    inner_width,
                    "38;5;244",
                ),
            );
            push_line(&mut out, &framed_left("", inner_width, "38;5;244"));
            push_line(
                &mut out,
                &framed_left(
                    "\x1b[38;5;244mEnter crear  ·  Esc cancelar\x1b[0m",
                    inner_width,
                    "38;5;244",
                ),
            );
        } else {
            let chrome_rows = 15usize;
            let available = (rows as usize).saturating_sub(chrome_rows).max(3);
            let selected = self.launcher.selected;
            let total = self.launcher.entries.len() + 1;
            let file_selected = selected.saturating_sub(2);
            let start = file_selected.saturating_sub(available.saturating_sub(1) / 2);
            let end = (start + available).min(total);

            for index in start..end {
                if index == 0 {
                    let selected_now = selected == 2;
                    let marker = if selected_now { "▶" } else { " " };
                    let line = format!(
                        "{}{} \x1b[38;5;222m{ICON_DOCUMENT}\x1b[0m   Nuevo archivo",
                        if selected_now { "\x1b[1;38;5;222m" } else { "" },
                        marker,
                    );
                    let line = if selected_now {
                        format!("{line}\x1b[0m")
                    } else {
                        line
                    };
                    push_line(&mut out, &framed_left(&line, inner_width, "38;5;244"));
                    continue;
                }

                if let Some(entry) = self.launcher.entries.get(index - 1) {
                    let selected_now = selected == index + 2;
                    let marker = if selected_now { "▶" } else { " " };
                    let suffix = if entry.directory { "/" } else { "" };
                    let (icon, icon_color) = file_icon(entry);
                    let max_name = inner_width.saturating_sub(8);
                    let name = truncate(&format!("{}{suffix}", entry.name), max_name);

                    let row = if selected_now {
                        format!(
                            "\x1b[1;38;5;117m{marker}\x1b[0m  \x1b[{icon_color}m{icon}\x1b[0m   \x1b[1;38;5;255m{name}\x1b[0m"
                        )
                    } else {
                        format!(
                            "{marker}  \x1b[{icon_color}m{icon}\x1b[0m   \x1b[38;5;250m{name}\x1b[0m"
                        )
                    };

                    push_line(&mut out, &framed_left(&row, inner_width, "38;5;244"));
                }
            }
        }

        push_line(
            &mut out,
            &format!("\x1b[38;5;244m├{}┤\x1b[0m", "─".repeat(inner_width)),
        );

        if let Some(message) = self.launcher.message.as_deref() {
            let message = truncate(message, inner_width.saturating_sub(2));
            push_line(
                &mut out,
                &framed_left(
                    &format!("\x1b[38;5;203m{message}\x1b[0m"),
                    inner_width,
                    "38;5;244",
                ),
            );
        } else if !self.launcher.creating {
            push_line(
                &mut out,
                &framed_left(
                    "\x1b[38;5;244m↑↓ seleccionar  Enter elegir/abrir  N nuevo  Z alternar modo  Backspace subir  R refrescar\x1b[0m",
                    inner_width,
                    "38;5;244",
                ),
            );
        }

        push_last_line(
            &mut out,
            &format!("\x1b[38;5;244m╰{}╯\x1b[0m", "─".repeat(inner_width)),
        );

        self.parser.process(out.as_bytes());
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
        let (left_pad, top_pad, cell_width, cell_height) = self.geometry();
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
                    fill_rect(pixels, (width, height), (x, y, w, h), bg);
                }

                let content = cell.contents();
                if content.is_empty() {
                    continue;
                }

                let mut pen_x = x;
                let baseline = y + (cell_height * 0.80).round() as i32;
                for ch in content.chars() {
                    if let Some(icon) = terminal_icon(ch) {
                        draw_terminal_icon(
                            pixels,
                            (width, height),
                            (pen_x, y),
                            (cell_width, cell_height),
                            icon,
                            fg,
                        );
                        pen_x += (cell_width * 2.0).round() as i32;
                        continue;
                    }

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

#[cfg(windows)]
fn slint_hwnd(ui: &ZenWindow) -> Option<HWND> {
    // Slint returns its own WindowHandle wrapper. That wrapper implements
    // raw-window-handle's HasWindowHandle trait.
    let handle = ui.window().window_handle();
    let window_handle = handle.window_handle().ok()?;
    let RawWindowHandle::Win32(win32) = window_handle.as_raw() else {
        return None;
    };
    Some(win32.hwnd.get() as HWND)
}

#[cfg(windows)]
unsafe fn focus_native_window(hwnd: HWND) {
    unsafe {
        let foreground = GetForegroundWindow();
        let current_thread = GetCurrentThreadId();
        let foreground_thread = if foreground.is_null() {
            0
        } else {
            GetWindowThreadProcessId(foreground, std::ptr::null_mut())
        };

        let attached = foreground_thread != 0
            && foreground_thread != current_thread
            && AttachThreadInput(current_thread, foreground_thread, 1) != 0;

        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        SetActiveWindow(hwnd);
        SetFocus(hwnd);

        if attached {
            AttachThreadInput(current_thread, foreground_thread, 0);
        }
    }
}

#[cfg(windows)]
unsafe fn set_zen_topmost(hwnd: HWND, enabled: bool) {
    unsafe {
        let insert_after = if enabled {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        if enabled {
            focus_native_window(hwnd);
        }
    }
}

pub fn run(initial: Option<PathBuf>, zen_requested: bool) -> Result<()> {
    BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .with_winit_window_attributes_hook(|attributes| attributes.with_decorations(false))
        .select()
        .map_err(|error| anyhow::anyhow!("No se pudo inicializar Winit/FemtoVG: {error}"))?;

    let model = std::rc::Rc::new(std::cell::RefCell::new(TerminalModel::new(
        initial,
        zen_requested,
    )?));
    let ui = ZenWindow::new()?;
    ui.set_version_text(format!("v{APP_VERSION}").into());

    {
        let model = model.clone();
        ui.on_key_input(move |text, ctrl, alt, shift| {
            handle_key(&mut model.borrow_mut(), text.as_str(), ctrl, alt, shift);
        });
    }

    {
        let model = model.clone();
        ui.on_toggle_zen(move || {
            model.borrow_mut().toggle_editor_zen();
        });
    }

    {
        let weak = ui.as_weak();
        let model = model.clone();
        ui.on_close_window(move || {
            // Real Zen Mode can only be left by closing Helix itself.
            // Native close requests such as Alt+F4 are ignored while engaged.
            if model.borrow().zen_engaged() {
                return;
            }
            if let Some(ui) = weak.upgrade() {
                let _ = ui.hide();
            }
            let _ = slint::quit_event_loop();
        });
    }

    ui.show()?;

    let weak = ui.as_weak();
    let timer = Timer::default();
    let last_zen = std::rc::Rc::new(std::cell::Cell::new(false));
    {
        let model = model.clone();
        let last_zen = last_zen.clone();
        timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };

            let size = ui.window().size();
            let scale = ui.window().scale_factor();

            let mut model = model.borrow_mut();
            model.resize(size.width, size.height, scale);
            model.tick();

            let zen = model.zen_engaged();
            ui.set_editor_active(model.editor.is_some());

            if zen != last_zen.get() {
                ui.set_zen_active(zen);
                ui.window().set_fullscreen(zen);

                #[cfg(windows)]
                if let Some(hwnd) = slint_hwnd(&ui) {
                    unsafe {
                        set_zen_topmost(hwnd, zen);
                    }
                }

                last_zen.set(zen);
                model.dirty = true;
            }

            #[cfg(windows)]
            if zen
                && let Some(hwnd) = slint_hwnd(&ui)
                && unsafe { GetForegroundWindow() != hwnd }
            {
                unsafe {
                    focus_native_window(hwnd);
                }
            }

            if model.dirty {
                ui.set_terminal_image(model.render());
            }
        });
    }

    slint::run_event_loop()?;
    timer.stop();
    Ok(())
}

fn handle_key(model: &mut TerminalModel, text: &str, ctrl: bool, alt: bool, shift: bool) {
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

    // Resolve named/special keys first. Slint encodes Return as LF (0x0A),
    // which also falls inside the ASCII control range used below for Ctrl+A..Z.
    // If control translation runs first, Enter becomes Ctrl+J.
    if let Some(code) = raw_key_code(text, shift) {
        model.key_event(KeyEvent::new(code, modifiers));
        return;
    }

    // Some Windows/Slint paths deliver Ctrl+letter as the ASCII control
    // character without setting control=true. Translate only values that were
    // not already recognized as special keys above.
    if let Some(ch) = text.chars().next()
        && text.chars().count() == 1
        && ('\x01'..='\x1a').contains(&ch)
    {
        modifiers |= KeyModifiers::CONTROL;
        let letter = (ch as u8 + b'a' - 1) as char;
        model.key_event(KeyEvent::new(KeyCode::Char(letter), modifiers));
    }
}

fn key_is(text: &str, key: slint::platform::Key) -> bool {
    let encoded: SharedString = key.into();
    text == encoded.as_str()
}

fn raw_key_code(text: &str, shift: bool) -> Option<KeyCode> {
    use slint::platform::Key;

    match text {
        "\r" | "\n" => return Some(KeyCode::Enter),
        "\x1b" => return Some(KeyCode::Esc),
        "\x08" | "\x7f" => return Some(KeyCode::Backspace),
        _ => {}
    }

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

fn file_icon(entry: &Entry) -> (char, &'static str) {
    if entry.directory {
        return (ICON_FOLDER, "38;5;109");
    }

    let name = entry.name.to_ascii_lowercase();
    let extension = entry
        .path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if name == "cargo.toml" || name == "cargo.lock" {
        return (ICON_RUST, "38;5;208");
    }
    if name.ends_with(".lock") || name.contains("artifact-lock") || name.contains("build-lock") {
        return (ICON_LOCK, "38;5;244");
    }

    match extension.as_str() {
        "" | "txt" | "text" | "md" | "markdown" | "rst" | "log" => (ICON_DOCUMENT, "38;5;255"),
        "pdf" => (ICON_PDF, "38;5;203"),
        "doc" | "docx" | "odt" | "rtf" | "xls" | "xlsx" | "ods" | "csv" | "ppt" | "pptx"
        | "odp" => (ICON_OFFICE, "38;5;75"),
        "rs" => (ICON_RUST, "38;5;208"),
        "c" | "h" | "cpp" | "hpp" | "cs" | "go" | "py" | "js" | "ts" | "tsx" | "jsx" | "html"
        | "css" | "scss" | "toml" | "yaml" | "yml" | "json" | "xml" | "sh" | "bash" | "ps1"
        | "bat" | "cmd" => (ICON_CODE, "38;5;114"),
        "exe" | "com" | "msi" => (ICON_EXECUTABLE, "38;5;75"),
        "dll" => (ICON_COMPONENT, "38;5;110"),
        "pdb" | "obj" | "lib" | "a" => (ICON_BUILD, "38;5;244"),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" => (ICON_IMAGE, "38;5;176"),
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => (ICON_AUDIO, "38;5;175"),
        "mp4" | "mkv" | "avi" | "mov" | "webm" => (ICON_VIDEO, "38;5;175"),
        "zip" | "7z" | "rar" | "tar" | "gz" | "xz" | "bz2" => (ICON_ARCHIVE, "38;5;179"),
        _ => (ICON_DOCUMENT, "38;5;250"),
    }
}

fn framed_left(content: &str, width: usize, border_color: &str) -> String {
    let visible = visible_width(content);
    let padding = width.saturating_sub(visible + 2);
    format!(
        "\x1b[{border_color}m│\x1b[0m {content}{} \x1b[{border_color}m│\x1b[0m",
        " ".repeat(padding)
    )
}

fn framed_center(content: &str, width: usize, border_color: &str) -> String {
    let visible = visible_width(content);
    let free = width.saturating_sub(visible);
    let left = free / 2;
    let right = free.saturating_sub(left);
    format!(
        "\x1b[{border_color}m│\x1b[0m{}{}{}\x1b[{border_color}m│\x1b[0m",
        " ".repeat(left),
        content,
        " ".repeat(right)
    )
}

fn visible_width(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut visible = 0;

    while index < bytes.len() {
        if bytes[index] == 0x1b && index + 1 < bytes.len() && bytes[index + 1] == b'[' {
            index += 2;
            while index < bytes.len() {
                let byte = bytes[index];
                index += 1;
                if (0x40..=0x7e).contains(&byte) {
                    break;
                }
            }
            continue;
        }

        if let Some(ch) = text[index..].chars().next() {
            visible += 1;
            index += ch.len_utf8();
        } else {
            break;
        }
    }

    visible
}

fn centered_frame_line(content: &str, terminal_width: usize, frame_width: usize) -> String {
    let inner_width = frame_width.saturating_sub(2);
    let margin = " ".repeat(terminal_width.saturating_sub(frame_width) / 2);
    format!(
        "{margin}{}",
        framed_center(content, inner_width, "38;5;244")
    )
}

fn push_last_line(out: &mut String, line: &str) {
    out.push_str(line);
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

#[derive(Clone, Copy)]
enum TerminalIcon {
    Folder,
    Document,
    Code,
    Executable,
    Component,
    Build,
    Image,
    Audio,
    Video,
    Archive,
    Pdf,
    Office,
    Lock,
    Rust,
}

fn terminal_icon(ch: char) -> Option<TerminalIcon> {
    Some(match ch {
        ICON_FOLDER => TerminalIcon::Folder,
        ICON_DOCUMENT => TerminalIcon::Document,
        ICON_CODE => TerminalIcon::Code,
        ICON_EXECUTABLE => TerminalIcon::Executable,
        ICON_COMPONENT => TerminalIcon::Component,
        ICON_BUILD => TerminalIcon::Build,
        ICON_IMAGE => TerminalIcon::Image,
        ICON_AUDIO => TerminalIcon::Audio,
        ICON_VIDEO => TerminalIcon::Video,
        ICON_ARCHIVE => TerminalIcon::Archive,
        ICON_PDF => TerminalIcon::Pdf,
        ICON_OFFICE => TerminalIcon::Office,
        ICON_LOCK => TerminalIcon::Lock,
        ICON_RUST => TerminalIcon::Rust,
        _ => return None,
    })
}

fn icon_shade(color: Rgb, numerator: u16, denominator: u16) -> Rgb {
    let scale = |component: u8| ((u16::from(component) * numerator / denominator).min(255)) as u8;
    Rgb(scale(color.0), scale(color.1), scale(color.2))
}

fn icon_bright(color: Rgb) -> Rgb {
    let lift = |component: u8| component.saturating_add(48);
    Rgb(lift(color.0), lift(color.1), lift(color.2))
}

struct IconCanvas<'a> {
    pixels: &'a mut [Rgba8Pixel],
    surface: (u32, u32),
    origin: (i32, i32),
    size: (i32, i32),
}

impl IconCanvas<'_> {
    fn rect(&mut self, grid: (i32, i32, i32, i32), color: Rgb) {
        let (gx, gy, gw, gh) = grid;
        let (origin_x, origin_y) = self.origin;
        let (icon_width, icon_height) = self.size;
        let x0 = origin_x + gx * icon_width / 16;
        let y0 = origin_y + gy * icon_height / 16;
        let x1 = origin_x + (gx + gw) * icon_width / 16;
        let y1 = origin_y + (gy + gh) * icon_height / 16;
        fill_rect(
            self.pixels,
            self.surface,
            (x0, y0, (x1 - x0).max(1), (y1 - y0).max(1)),
            color,
        );
    }
}

fn draw_terminal_icon(
    pixels: &mut [Rgba8Pixel],
    surface: (u32, u32),
    origin: (i32, i32),
    cell_size: (f32, f32),
    icon: TerminalIcon,
    color: Rgb,
) {
    let (cell_width, cell_height) = cell_size;
    let iw = (cell_width * 2.0).round().max(12.0) as i32;
    let ih = cell_height.round().max(14.0) as i32;
    let mut canvas = IconCanvas {
        pixels,
        surface,
        origin,
        size: (iw, ih),
    };
    let dark = icon_shade(color, 2, 5);
    let mid = icon_shade(color, 3, 4);
    let bright = icon_bright(color);

    match icon {
        // Data cassette / directory module with raised tab.
        TerminalIcon::Folder => {
            canvas.rect((2, 3, 6, 2), bright);
            canvas.rect((1, 5, 14, 9), color);
            canvas.rect((2, 6, 12, 7), dark);
            canvas.rect((3, 8, 9, 1), bright);
            canvas.rect((11, 11, 2, 2), mid);
        }

        // Technical document plate with clipped corner and scan lines.
        TerminalIcon::Document | TerminalIcon::Pdf | TerminalIcon::Office => {
            canvas.rect((3, 1, 10, 14), color);
            canvas.rect((4, 2, 8, 12), dark);
            canvas.rect((10, 1, 3, 3), bright);
            canvas.rect((5, 6, 6, 1), bright);
            canvas.rect((5, 9, 5, 1), mid);
            canvas.rect((5, 12, 4, 1), mid);
            if matches!(icon, TerminalIcon::Pdf) {
                canvas.rect((4, 13, 8, 1), bright);
            } else if matches!(icon, TerminalIcon::Office) {
                canvas.rect((8, 5, 1, 8), bright);
            }
        }

        // Microterminal module: frame + angular prompt chevrons.
        TerminalIcon::Code => {
            canvas.rect((1, 2, 14, 12), color);
            canvas.rect((2, 3, 12, 10), dark);
            canvas.rect((4, 6, 2, 1), bright);
            canvas.rect((5, 7, 2, 1), bright);
            canvas.rect((4, 8, 2, 1), bright);
            canvas.rect((9, 9, 3, 1), mid);
        }

        // Executable as a glowing computational core.
        TerminalIcon::Executable => {
            canvas.rect((4, 2, 8, 12), color);
            canvas.rect((2, 5, 12, 6), color);
            canvas.rect((5, 4, 6, 8), dark);
            canvas.rect((6, 6, 4, 4), bright);
            canvas.rect((7, 7, 2, 2), mid);
        }

        // Plug-in board with connector pins.
        TerminalIcon::Component => {
            canvas.rect((3, 4, 10, 8), color);
            canvas.rect((4, 5, 8, 6), dark);
            for pin_y in [5, 8, 11] {
                canvas.rect((1, pin_y, 2, 1), bright);
                canvas.rect((13, pin_y, 2, 1), bright);
            }
            canvas.rect((6, 7, 4, 2), mid);
        }

        // Wireframe build block / package.
        TerminalIcon::Build => {
            canvas.rect((4, 3, 8, 2), bright);
            canvas.rect((2, 5, 12, 8), color);
            canvas.rect((3, 6, 10, 6), dark);
            canvas.rect((7, 5, 2, 8), mid);
            canvas.rect((3, 8, 10, 1), bright);
        }

        // CRT image frame: horizon and synthetic sun.
        TerminalIcon::Image => {
            canvas.rect((1, 2, 14, 12), color);
            canvas.rect((2, 3, 12, 10), dark);
            canvas.rect((10, 5, 2, 2), bright);
            canvas.rect((3, 10, 10, 1), mid);
            canvas.rect((4, 9, 3, 1), bright);
            canvas.rect((7, 8, 3, 2), color);
        }

        // Oscilloscope / waveform.
        TerminalIcon::Audio => {
            canvas.rect((1, 3, 14, 10), dark);
            for (gx, gy, gh) in [(3, 7, 3), (5, 5, 6), (7, 3, 10), (9, 5, 6), (11, 7, 3)] {
                canvas.rect((gx, gy, 1, gh), bright);
            }
        }

        // Small monitor with angular play marker.
        TerminalIcon::Video => {
            canvas.rect((1, 2, 14, 11), color);
            canvas.rect((2, 3, 12, 9), dark);
            canvas.rect((6, 5, 2, 6), bright);
            canvas.rect((8, 6, 2, 4), bright);
            canvas.rect((10, 7, 1, 2), bright);
            canvas.rect((5, 14, 6, 1), mid);
        }

        // Cartridge stack.
        TerminalIcon::Archive => {
            for gy in [3, 7, 11] {
                canvas.rect((2, gy, 12, 3), color);
                canvas.rect((3, gy + 1, 8, 1), dark);
                canvas.rect((12, gy + 1, 1, 1), bright);
            }
        }

        // Security module with hard-edged shackle.
        TerminalIcon::Lock => {
            canvas.rect((5, 2, 6, 2), color);
            canvas.rect((4, 4, 2, 4), color);
            canvas.rect((10, 4, 2, 4), color);
            canvas.rect((3, 7, 10, 7), color);
            canvas.rect((4, 8, 8, 5), dark);
            canvas.rect((7, 9, 2, 3), bright);
        }

        // Rust/Cargo shown as a gear-like reactor ring.
        TerminalIcon::Rust => {
            canvas.rect((6, 1, 4, 2), color);
            canvas.rect((6, 13, 4, 2), color);
            canvas.rect((1, 6, 2, 4), color);
            canvas.rect((13, 6, 2, 4), color);
            canvas.rect((3, 3, 10, 10), color);
            canvas.rect((4, 4, 8, 8), dark);
            canvas.rect((6, 6, 4, 4), bright);
            canvas.rect((7, 7, 2, 2), mid);
        }
    }
}

fn fill_rect(
    pixels: &mut [Rgba8Pixel],
    surface: (u32, u32),
    rect: (i32, i32, i32, i32),
    color: Rgb,
) {
    let (width, height) = surface;
    let (x, y, w, h) = rect;
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
