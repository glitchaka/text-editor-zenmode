use std::{
    fs,
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use zip::ZipArchive;

use crate::{document, pty_protocol::Replies};

pub const HELIX_UPSTREAM_VERSION: &str = "25.07.1";

#[cfg(windows)]
static HELIX_ARCHIVE: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/helix-25.07.1-x86_64-windows.zip"
));

struct Install {
    hx: PathBuf,
    runtime: PathBuf,
    config: PathBuf,
    appdata: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

struct NativeBuffer {
    source: PathBuf,
    edit: PathBuf,
    last_stamp: Option<FileStamp>,
    last_body: String,
}

pub struct EditorSession {
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    protocol: Arc<Mutex<vt100::Parser<Replies>>>,
    output: mpsc::Receiver<io::Result<Vec<u8>>>,
    finished: mpsc::Receiver<u32>,
    previous_was_cr: bool,
    startup_language_at: Option<Instant>,
    startup_language_sent: bool,
    native: Option<NativeBuffer>,
    native_temp_root: Option<PathBuf>,
}

impl EditorSession {
    pub fn start(file: &Path, cols: u16, rows: u16) -> Result<Self> {
        let install = ensure_installed(file)?;
        let (open_file, native, native_temp_root) = prepare_open_file(file)?;
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut command = CommandBuilder::new(&install.hx);
        command.cwd(
            file.parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        );
        command.env("HELIX_RUNTIME", install.runtime.to_string_lossy().as_ref());
        command.env("APPDATA", install.appdata.to_string_lossy().as_ref());
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.arg("--config");
        command.arg(&install.config);
        command.arg("--log");
        command.arg(install.config.with_file_name("helix.log"));
        command.arg(&open_file);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
        let protocol = Arc::new(Mutex::new(vt100::Parser::new_with_callbacks(
            rows,
            cols,
            0,
            Replies::default(),
        )));
        let reply_writer = writer.clone();
        let reply_protocol = protocol.clone();
        let (output_tx, output) = mpsc::channel::<io::Result<Vec<u8>>>();

        // Helix can request a cursor report while it is still starting.
        // Parse and answer terminal queries in the reader thread before
        // spawn_command() returns, matching the working SST integration.
        thread::spawn(move || {
            let mut buffer = [0u8; 16 * 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        let replies = {
                            let mut parser =
                                reply_protocol.lock().unwrap_or_else(|e| e.into_inner());
                            parser.process(&buffer[..count]);
                            std::mem::take(&mut parser.callbacks_mut().bytes)
                        };

                        if !replies.is_empty() {
                            let mut writer = reply_writer.lock().unwrap_or_else(|e| e.into_inner());
                            if let Err(error) =
                                writer.write_all(&replies).and_then(|_| writer.flush())
                            {
                                let _ = output_tx.send(Err(error));
                                break;
                            }
                        }

                        if output_tx.send(Ok(buffer[..count].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = output_tx.send(Err(error));
                        break;
                    }
                }
            }
        });

        let mut child = pair.slave.spawn_command(command)?;
        drop(pair.slave);

        let (finished_tx, finished) = mpsc::channel();
        thread::spawn(move || {
            let code = child.wait().map(|status| status.exit_code()).unwrap_or(1);
            let _ = finished_tx.send(code);
        });

        Ok(Self {
            master: pair.master,
            writer,
            protocol,
            output,
            finished,
            previous_was_cr: false,
            startup_language_at: file
                .extension()
                .is_none()
                .then(|| Instant::now() + Duration::from_millis(350)),
            startup_language_sent: false,
            native,
            native_temp_root,
        })
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.protocol
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .screen_mut()
            .set_size(rows, cols);
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    pub fn try_output(&self) -> Option<io::Result<Vec<u8>>> {
        self.output.try_recv().ok()
    }

    pub fn exit_status(&self) -> Option<u32> {
        self.finished.try_recv().ok()
    }

    pub fn settle_startup_language(&mut self) -> Result<()> {
        if self.startup_language_sent {
            return Ok(());
        }

        let Some(when) = self.startup_language_at else {
            return Ok(());
        };
        if Instant::now() < when {
            return Ok(());
        }

        // Extensionless prose files are a first-class Zenmode use case.
        // Force Helix onto the managed "prose" language once startup has settled.
        // Route the synthetic command through the same keyboard encoder as real
        // input so it also works after Helix enables Windows input mode (?9001h).
        self.send_command(":set-language prose")?;
        self.startup_language_sent = true;
        Ok(())
    }

    pub fn write_reply(&self, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        writer.write_all(bytes)?;
        writer.flush()?;
        Ok(())
    }

    pub fn win32_input(&self) -> bool {
        self.protocol
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .callbacks()
            .win32_input
    }

    pub fn send_key(&self, key: KeyEvent, win32: bool) -> Result<()> {
        let Some(bytes) = encode_input(key, win32) else {
            return Ok(());
        };
        self.write_reply(&bytes)
    }

    pub fn send_command(&self, command: &str) -> Result<()> {
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

    pub fn sync_native(&mut self) -> Result<bool> {
        self.sync_native_inner(false)
    }

    pub fn flush_native(&mut self) -> Result<bool> {
        self.sync_native_inner(true)
    }

    pub fn open_source(&mut self, source: &Path) -> Result<()> {
        self.flush_native()?;

        if document::is_native_path(source) {
            let root = self
                .native_temp_root
                .get_or_insert_with(native_temp_root)
                .clone();
            let native = prepare_native_buffer(source, &root)?;
            let command = format!(":open \"{}\"", helix_path(&native.edit));
            self.send_command(&command)?;
            self.native = Some(native);
        } else {
            let command = format!(":open \"{}\"", helix_path(source));
            self.send_command(&command)?;
            self.native = None;
        }

        Ok(())
    }

    fn sync_native_inner(&mut self, force: bool) -> Result<bool> {
        let Some(native) = self.native.as_ref() else {
            return Ok(false);
        };

        let stamp = file_stamp(&native.edit);
        if !force && stamp == native.last_stamp {
            return Ok(false);
        }

        let source = native.source.clone();
        let edit = native.edit.clone();
        let previous_body = native.last_body.clone();
        let body = fs::read_to_string(&edit)
            .with_context(|| format!("No se pudo leer el cuerpo editable {}", edit.display()))?;
        let mut document = document::read(&source)?;
        let changed = document.body != body;

        if changed {
            document.formatting =
                document::remap_formatting(&document.formatting, &previous_body, &body);
            document.body = body.clone();
            document::write(&source, &document)
                .with_context(|| format!("No se pudo guardar {}", source.display()))?;
        }

        if let Some(native) = self.native.as_mut() {
            native.last_stamp = stamp;
            native.last_body = body;
        }

        Ok(changed)
    }

    pub fn paste(&self, text: &str, ensure_insert: bool, wrap_width: Option<usize>) -> Result<()> {
        let win32 = self.win32_input();
        let prepared = match wrap_width {
            Some(width) => wrap_pasted_text(text, width),
            None => normalize_pasted_text(text),
        };
        let mut bytes = Vec::new();

        if ensure_insert {
            if let Some(encoded) =
                encode_input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), win32)
            {
                bytes.extend_from_slice(&encoded);
            }
            if let Some(encoded) =
                encode_input(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), win32)
            {
                bytes.extend_from_slice(&encoded);
            }
        }

        bytes.extend_from_slice(&encode_paste(&prepared, win32));

        let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        writer.write_all(&bytes)?;
        writer.flush()?;
        Ok(())
    }

    pub fn normalize_output(&mut self, bytes: &[u8]) -> Vec<u8> {
        let extra = bytes.iter().filter(|&&byte| byte == b'\n').count();
        let mut normalized = Vec::with_capacity(bytes.len() + extra);
        for &byte in bytes {
            if byte == b'\n' && !self.previous_was_cr {
                normalized.push(b'\r');
            }
            normalized.push(byte);
            self.previous_was_cr = byte == b'\r';
        }
        normalized
    }
}

fn prepare_open_file(file: &Path) -> Result<(PathBuf, Option<NativeBuffer>, Option<PathBuf>)> {
    if !document::is_native_path(file) {
        return Ok((file.to_path_buf(), None, None));
    }

    let root = native_temp_root();
    let native = prepare_native_buffer(file, &root)?;
    let edit = native.edit.clone();
    Ok((edit, Some(native), Some(root)))
}

fn prepare_native_buffer(source: &Path, root: &Path) -> Result<NativeBuffer> {
    let document = document::read(source)?;
    let id = safe_component(&document.metadata.id);
    let directory = root.join(id);
    fs::create_dir_all(&directory)?;

    let file_name = source
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| std::ffi::OsStr::new("Documento.hsst"));
    let edit = directory.join(file_name);
    fs::write(&edit, document.body.as_bytes())
        .with_context(|| format!("No se pudo preparar {}", source.display()))?;
    let last_stamp = file_stamp(&edit);
    let last_body = document.body;

    Ok(NativeBuffer {
        source: source.to_path_buf(),
        edit,
        last_stamp,
        last_body,
    })
}

fn native_temp_root() -> PathBuf {
    std::env::temp_dir()
        .join("helix-sst-zen")
        .join(std::process::id().to_string())
}

fn safe_component(value: &str) -> String {
    let filtered = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .collect::<String>();
    if filtered.is_empty() {
        "document".into()
    } else {
        filtered
    }
}

fn file_stamp(path: &Path) -> Option<FileStamp> {
    let metadata = fs::metadata(path).ok()?;
    Some(FileStamp {
        modified: metadata.modified().ok(),
        len: metadata.len(),
    })
}

fn helix_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}

impl Drop for EditorSession {
    fn drop(&mut self) {
        let _ = self.flush_native();
        if let Some(root) = self.native_temp_root.as_ref() {
            let _ = fs::remove_dir_all(root);
        }
    }
}

#[cfg(windows)]
fn ensure_installed(current_file: &Path) -> Result<Install> {
    let launcher = std::env::current_exe()?;
    let exe_dir = launcher
        .parent()
        .context("No se pudo determinar el directorio de Helix-SST Zenmode")?
        .to_path_buf();

    let root = exe_dir
        .join("data")
        .join("helix-sst")
        .join(HELIX_UPSTREAM_VERSION);
    let config_dir = exe_dir.join("config");
    let marker = root.join(".installed");

    let mut resolved = read_install_marker(&root, &marker);

    if resolved.is_none() {
        fs::create_dir_all(&root)?;

        let existing_hx = find_named(&root, "hx.exe", false);
        let existing_runtime = find_named(&root, "runtime", true);

        if existing_hx.is_none() || existing_runtime.is_none() {
            let cursor = Cursor::new(HELIX_ARCHIVE);
            let mut archive = ZipArchive::new(cursor)
                .context("El paquete embebido de Helix no es un ZIP válido")?;

            for index in 0..archive.len() {
                let mut entry = archive.by_index(index)?;
                let Some(relative) = entry.enclosed_name().map(Path::to_path_buf) else {
                    continue;
                };
                let destination = root.join(relative);

                if entry.is_dir() {
                    fs::create_dir_all(&destination)?;
                    continue;
                }

                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }

                let mut output = fs::File::create(&destination)?;
                std::io::copy(&mut entry, &mut output)?;
            }
        }

        let hx =
            find_named(&root, "hx.exe", false).context("El paquete de Helix no contiene hx.exe")?;
        let runtime = find_named(&root, "runtime", true)
            .context("El paquete de Helix no contiene runtime")?;

        write_install_marker(&root, &marker, &hx, &runtime)?;
        resolved = Some((hx, runtime));
    }

    let (hx, runtime) = resolved.context("No se pudieron resolver las rutas de Helix")?;
    fs::create_dir_all(&config_dir)?;

    let config = config_dir.join("config.toml");
    let appdata = config_dir.join("appdata");
    let helix_config = appdata.join("helix");
    fs::create_dir_all(&helix_config)?;

    write_editor_config(&config, &launcher)?;
    let themes_dir = helix_config.join("themes");
    fs::create_dir_all(&themes_dir)?;
    write_zen_theme(&themes_dir.join("helix-sst-zen.toml"))?;
    write_language_config(
        &helix_config.join("languages.toml"),
        &launcher,
        &config_dir.join(".spell-user"),
        current_file,
    )?;

    fs::write(
        root.join("HELIX-SST-SPELL-DICTIONARY-LICENSE.txt"),
        crate::spell::DICTIONARY_LICENSE,
    )?;

    Ok(Install {
        hx,
        runtime,
        config,
        appdata,
    })
}

#[cfg(not(windows))]
fn ensure_installed(_current_file: &Path) -> Result<Install> {
    anyhow::bail!("Helix-SST Zenmode está empaquetado actualmente para Windows")
}

#[cfg(windows)]
fn read_install_marker(root: &Path, marker: &Path) -> Option<(PathBuf, PathBuf)> {
    let content = fs::read_to_string(marker).ok()?;
    let mut hx = None;
    let mut runtime = None;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("hx=") {
            hx = Some(root.join(value));
        } else if let Some(value) = line.strip_prefix("runtime=") {
            runtime = Some(root.join(value));
        }
    }

    let hx = hx?;
    let runtime = runtime?;
    (hx.is_file() && runtime.is_dir()).then_some((hx, runtime))
}

#[cfg(windows)]
fn write_install_marker(root: &Path, marker: &Path, hx: &Path, runtime: &Path) -> Result<()> {
    let hx = hx
        .strip_prefix(root)
        .unwrap_or(hx)
        .to_string_lossy()
        .replace('\\', "/");
    let runtime = runtime
        .strip_prefix(root)
        .unwrap_or(runtime)
        .to_string_lossy()
        .replace('\\', "/");

    fs::write(
        marker,
        format!("helix-upstream={HELIX_UPSTREAM_VERSION}\nhx={hx}\nruntime={runtime}\n"),
    )?;
    Ok(())
}

fn write_editor_config(path: &Path, launcher: &Path) -> Result<()> {
    let exe = toml_path(launcher);
    let content = format!(
        r#"theme = "helix-sst-zen"

[editor]
line-number = "absolute"
mouse = false
true-color = true
cursorline = true
bufferline = "multiple"
color-modes = true
auto-completion = true
end-of-line-diagnostics = "disable"

[editor.inline-diagnostics]
cursor-line = "warning"
other-lines = "disable"

[editor.statusline]
left = ["mode", "spinner", "file-base-name", "file-modification-indicator"]
center = []
right = ["diagnostics", "selections", "position", "file-encoding", "file-type"]

[editor.statusline.mode]
normal = "NORMAL · i: escribir · F2: ortografía"
insert = "INSERTAR · Alt+d: — · F2: ortografía · Esc: comandos"
select = "SELECCIÓN · Esc: normal"

[editor.clipboard-provider.custom]
yank = {{ command = "{exe}", args = ["--clipboard-get"] }}
paste = {{ command = "{exe}", args = ["--clipboard-set"] }}

[keys.normal]
F2 = "code_action"
C-left = "move_prev_word_start"
C-right = "move_next_word_start"
C-z = "undo"
C-y = "redo"
C-S-z = "redo"
C-a = ["select_all", "select_mode"]
F13 = "move_prev_word_start"
F14 = "move_next_word_start"
F17 = "undo"
F18 = "redo"
F19 = ["select_all", "select_mode"]

[keys.insert]
F2 = "code_action"
A-d = "@—"
C-g = "@—"
C-left = "move_prev_word_start"
C-right = "move_next_word_start"
C-backspace = "delete_word_backward"
C-del = "delete_word_forward"
C-z = ["normal_mode", "undo", "insert_mode"]
C-y = ["normal_mode", "redo", "insert_mode"]
C-S-z = ["normal_mode", "redo", "insert_mode"]
C-a = ["normal_mode", "select_all", "select_mode"]
F13 = "move_prev_word_start"
F14 = "move_next_word_start"
F15 = "delete_word_backward"
F16 = "delete_word_forward"
F17 = ["normal_mode", "undo", "insert_mode"]
F18 = ["normal_mode", "redo", "insert_mode"]
F19 = ["normal_mode", "select_all", "select_mode"]

[keys.select]
F2 = "code_action"
C-left = "extend_prev_word_start"
C-right = "extend_next_word_start"
C-z = "undo"
C-y = "redo"
C-S-z = "redo"
C-a = "select_all"
F13 = "extend_prev_word_start"
F14 = "extend_next_word_start"
F17 = "undo"
F18 = "redo"
F19 = "select_all"
"#
    );
    fs::write(path, content)?;
    Ok(())
}

fn write_zen_theme(path: &Path) -> Result<()> {
    fs::write(
        path,
        r##"inherits = "gruvbox"

# HSST semantic tokens. The prose LSP uses these scopes only for native
# writing markup; the rest of Gruvbox remains inherited.
"comment" = { fg = "#928374", modifiers = ["dim"] }
"keyword" = { fg = "#fabd2f", modifiers = ["bold"] }
"string" = { fg = "#ebdbb2", modifiers = ["bold"] }
"regexp" = { fg = "#d3869b", modifiers = ["italic"] }
"macro" = { fg = "#282828", bg = "#fabd2f", modifiers = ["bold"] }
"##,
    )?;
    Ok(())
}

fn write_language_config(
    path: &Path,
    launcher: &Path,
    user_dictionary: &Path,
    current_file: &Path,
) -> Result<()> {
    let launcher = toml_path(launcher);
    let user_dictionary = toml_path(user_dictionary);
    let source_file = toml_path(current_file);
    let library_root = toml_path(
        current_file
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new(".")),
    );

    // Helix treats a string in file-types as an extension, or as the complete
    // filename when the path has no extension. Register the current extensionless
    // document explicitly so prose files such as "Pensamentao" still get the
    // Helix-SST spelling LSP without forcing a .txt suffix.
    let extensionless_name = if current_file.extension().is_none() {
        current_file
            .file_name()
            .and_then(|name| name.to_str())
            .map(toml_string)
    } else {
        None
    };

    let text_file_types = match extensionless_name {
        Some(name) => {
            format!("[\"hsst\", \"txt\", \"text\", \"{name}\", {{ glob = \"*/{name}\" }}]")
        }
        None => "[\"hsst\", \"txt\", \"text\"]".to_owned(),
    };

    let content = format!(
        r#"[language-server.helix-sst-spell]
command = "{launcher}"
args = ["--helix-sst-spell", "{user_dictionary}", "{library_root}", "{source_file}"]

[[language]]
name = "prose"
scope = "text.plain"
file-types = {text_file_types}
language-servers = ["helix-sst-spell"]

[[language]]
name = "markdown"
language-servers = ["helix-sst-spell"]
"#
    );
    fs::write(path, content)?;
    Ok(())
}

fn toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}

#[cfg(windows)]
fn find_named(root: &Path, name: &str, directory: bool) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let matches_kind = if directory {
            path.is_dir()
        } else {
            path.is_file()
        };
        if matches_kind
            && path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(name))
        {
            return Some(path);
        }
        if path.is_dir()
            && let Some(found) = find_named(&path, name, directory)
        {
            return Some(found);
        }
    }
    None
}

fn normalize_pasted_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                normalized.push('\n');
            }
            // Word can expose manual line/paragraph separators through
            // plain-text clipboard formats instead of CR/LF.
            '\u{000B}' | '\u{000C}' | '\u{0085}' | '\u{2028}' | '\u{2029}' => {
                normalized.push('\n');
            }
            other => normalized.push(other),
        }
    }

    normalized
}

fn wrap_pasted_text(text: &str, width: usize) -> String {
    let normalized = normalize_pasted_text(text);
    if width == 0 {
        return normalized;
    }

    let mut output = String::with_capacity(normalized.len());
    for (line_index, line) in normalized.split('\n').enumerate() {
        if line_index > 0 {
            output.push('\n');
        }

        let mut remaining = line;
        while remaining.chars().count() > width {
            let boundary = remaining
                .char_indices()
                .nth(width)
                .map(|(index, _)| index)
                .unwrap_or(remaining.len());
            let prefix = &remaining[..boundary];
            let whitespace = prefix
                .char_indices()
                .rev()
                .find(|(_, ch)| ch.is_whitespace())
                .map(|(index, _)| index);

            if let Some(split) = whitespace.filter(|split| *split > 0) {
                output.push_str(prefix[..split].trim_end());
                output.push('\n');
                remaining = remaining[split..].trim_start_matches(char::is_whitespace);
            } else {
                output.push_str(prefix);
                output.push('\n');
                remaining = &remaining[boundary..];
            }
        }
        output.push_str(remaining);
    }
    output
}

fn encode_command_colon(win32: bool) -> Vec<u8> {
    if !win32 {
        return b":".to_vec();
    }

    #[cfg(windows)]
    {
        const VK_OEM_1: u16 = 0xBA;
        const SHIFT_PRESSED: u32 = 0x10;
        let scan = unsafe {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::MapVirtualKeyW(VK_OEM_1.into(), 0)
        };
        let mut bytes = Vec::new();
        for down in [1, 0] {
            bytes.extend_from_slice(
                format!("\x1b[{VK_OEM_1};{scan};58;{down};{SHIFT_PRESSED};1_").as_bytes(),
            );
        }
        bytes
    }

    #[cfg(not(windows))]
    b":".to_vec()
}

fn encode_paste(text: &str, win32: bool) -> Vec<u8> {
    let normalized = normalize_pasted_text(text);

    if !win32 {
        let mut bytes = Vec::with_capacity(normalized.len() + 12);
        bytes.extend_from_slice(b"\x1b[200~");
        bytes.extend_from_slice(normalized.as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
        return bytes;
    }

    let mut bytes = Vec::with_capacity(normalized.len() * 8);
    for ch in normalized.chars() {
        let code = match ch {
            '\n' => KeyCode::Enter,
            '\t' => KeyCode::Tab,
            other => KeyCode::Char(other),
        };
        if let Some(encoded) = encode_input(KeyEvent::new(code, KeyModifiers::NONE), true) {
            bytes.extend_from_slice(&encoded);
        }
    }
    bytes
}

fn encode_input(key: KeyEvent, win32: bool) -> Option<Vec<u8>> {
    if !win32 {
        return encode_key(key);
    }
    if key.kind == KeyEventKind::Release {
        return None;
    }

    let mut state = 0u32;
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        state |= 0x10;
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        state |= 0x02;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        state |= 0x08;
    }

    let (vk, text): (u16, String) = match key.code {
        KeyCode::Char(ch) => {
            let vk = if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase() as u16
            } else {
                0
            };
            let character = if key.modifiers.contains(KeyModifiers::CONTROL) {
                match ch.to_ascii_lowercase() {
                    'a'..='z' => char::from((ch.to_ascii_lowercase() as u8) & 0x1f),
                    ' ' | '@' | '2' => '\0',
                    '[' => '\x1b',
                    '\\' => '\x1c',
                    ']' => '\x1d',
                    '^' => '\x1e',
                    '_' => '\x1f',
                    '?' => '\x7f',
                    _ => ch,
                }
            } else {
                ch
            };
            (vk, character.to_string())
        }
        KeyCode::Enter => (0x0d, "\r".into()),
        KeyCode::Esc => (0x1b, "\x1b".into()),
        KeyCode::Backspace => (0x08, "\x08".into()),
        KeyCode::Tab => (0x09, "\t".into()),
        KeyCode::BackTab => {
            state |= 0x10;
            (0x09, "\t".into())
        }
        KeyCode::Left => (0x25, String::new()),
        KeyCode::Up => (0x26, String::new()),
        KeyCode::Right => (0x27, String::new()),
        KeyCode::Down => (0x28, String::new()),
        KeyCode::Home => (0x24, String::new()),
        KeyCode::End => (0x23, String::new()),
        KeyCode::PageUp => (0x21, String::new()),
        KeyCode::PageDown => (0x22, String::new()),
        KeyCode::Insert => (0x2d, String::new()),
        KeyCode::Delete => (0x2e, String::new()),
        KeyCode::F(number @ 1..=24) => (0x70 + u16::from(number) - 1, String::new()),
        _ => return None,
    };

    if matches!(vk, 0x21..=0x28 | 0x2d | 0x2e) {
        state |= 0x100;
    }

    #[cfg(windows)]
    let scan =
        unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::MapVirtualKeyW(vk.into(), 0) };
    #[cfg(not(windows))]
    let scan = 0;

    let units: Vec<u16> = if text.is_empty() {
        vec![0]
    } else {
        text.encode_utf16().collect()
    };

    let mut bytes = Vec::new();
    for down in [1, 0] {
        for &unit in &units {
            let unit = if down == 0 && units.len() > 1 {
                0
            } else {
                unit
            };
            bytes
                .extend_from_slice(format!("\x1b[{vk};{scan};{unit};{down};{state};1_").as_bytes());
        }
    }
    Some(bytes)
}

fn encode_key(key: KeyEvent) -> Option<Vec<u8>> {
    if key.kind == KeyEventKind::Release {
        return None;
    }

    let modifiers = key.modifiers;
    let alt = modifiers.contains(KeyModifiers::ALT);
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let shift = modifiers.contains(KeyModifiers::SHIFT);

    let mut bytes = match key.code {
        KeyCode::Char(ch) if ctrl => {
            let lower = ch.to_ascii_lowercase();
            let code = match lower {
                'a'..='z' => (lower as u8) & 0x1f,
                '[' => 0x1b,
                '\\' => 0x1c,
                ']' => 0x1d,
                '^' => 0x1e,
                '_' => 0x1f,
                '?' => 0x7f,
                ' ' | '2' | '@' => 0,
                _ => return None,
            };
            vec![code]
        }
        KeyCode::Char(ch) => ch.to_string().into_bytes(),
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => b"\t".to_vec(),
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Up => csi_key('A', shift, alt, ctrl),
        KeyCode::Down => csi_key('B', shift, alt, ctrl),
        KeyCode::Right => csi_key('C', shift, alt, ctrl),
        KeyCode::Left => csi_key('D', shift, alt, ctrl),
        KeyCode::Home => csi_tilde_or_simple("H", "1~", shift, alt, ctrl),
        KeyCode::End => csi_tilde_or_simple("F", "4~", shift, alt, ctrl),
        KeyCode::Insert => csi_tilde("2~", shift, alt, ctrl),
        KeyCode::Delete => csi_tilde("3~", shift, alt, ctrl),
        KeyCode::PageUp => csi_tilde("5~", shift, alt, ctrl),
        KeyCode::PageDown => csi_tilde("6~", shift, alt, ctrl),
        KeyCode::F(number) => function_key(number, shift, alt, ctrl)?,
        _ => return None,
    };

    if alt && matches!(key.code, KeyCode::Char(_)) {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}

fn modifier_code(shift: bool, alt: bool, ctrl: bool) -> u8 {
    1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl)
}

fn csi_key(final_char: char, shift: bool, alt: bool, ctrl: bool) -> Vec<u8> {
    let modifier = modifier_code(shift, alt, ctrl);
    if modifier == 1 {
        format!("\x1b[{final_char}").into_bytes()
    } else {
        format!("\x1b[1;{modifier}{final_char}").into_bytes()
    }
}

fn csi_tilde(sequence: &str, shift: bool, alt: bool, ctrl: bool) -> Vec<u8> {
    let modifier = modifier_code(shift, alt, ctrl);
    if modifier == 1 {
        format!("\x1b[{sequence}").into_bytes()
    } else {
        let stem = sequence.trim_end_matches('~');
        format!("\x1b[{stem};{modifier}~").into_bytes()
    }
}

fn csi_tilde_or_simple(simple: &str, tilde: &str, shift: bool, alt: bool, ctrl: bool) -> Vec<u8> {
    if modifier_code(shift, alt, ctrl) == 1 {
        format!("\x1b[{simple}").into_bytes()
    } else {
        csi_tilde(tilde, shift, alt, ctrl)
    }
}

fn function_key(number: u8, shift: bool, alt: bool, ctrl: bool) -> Option<Vec<u8>> {
    let base = match number {
        1 => "11~",
        2 => "12~",
        3 => "13~",
        4 => "14~",
        5 => "15~",
        6 => "17~",
        7 => "18~",
        8 => "19~",
        9 => "20~",
        10 => "21~",
        11 => "23~",
        12 => "24~",
        13 => "25~",
        14 => "26~",
        15 => "28~",
        16 => "29~",
        17 => "31~",
        18 => "32~",
        19 => "33~",
        _ => return None,
    };
    Some(csi_tilde(base, shift, alt, ctrl))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_hsst_shadow_contains_only_body() {
        let root = std::env::temp_dir().join(format!("helix-sst-shadow-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("debe crear temp");

        let source = root.join("Capítulo 1.hsst");
        let document = document::HsstDocument {
            metadata: document::DocumentMetadata {
                format: 1,
                id: "shadow-test".into(),
                title: "Capítulo 1".into(),
                project: "Puerto Ámbar".into(),
                kind: "chapter".into(),
                chapter: Some(1),
                order: 10,
                language: "es-CL".into(),
                status: "draft".into(),
                page: crate::page::PageProfile::default(),
            },
            body: "Primera línea.\nSegunda línea.\n".into(),
            formatting: Vec::new(),
        };
        fs::write(&source, document::serialize(&document)).expect("debe crear HSST");

        let shadow_root = root.join("shadow");
        let native = prepare_native_buffer(&source, &shadow_root).expect("debe preparar buffer");
        let editable = fs::read_to_string(&native.edit).expect("debe leer buffer");

        assert_eq!(editable, document.body);
        assert!(!editable.contains("format ="));
        assert!(!editable.contains("project ="));
        assert!(!editable.contains("+++"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pasted_word_manual_breaks_become_real_lines() {
        let input = "Uno\u{000B}Dos\rTres\r\nCuatro\u{0085}Cinco\u{2028}Seis\u{2029}Siete";
        assert_eq!(
            normalize_pasted_text(input),
            "Uno\nDos\nTres\nCuatro\nCinco\nSeis\nSiete"
        );
    }

    #[test]
    fn pasted_prose_preserves_original_paragraph_structure() {
        let input = "Uno dos tres cuatro cinco seis siete ocho nueve diez.\r\n\r\nSegundo párrafo.";
        assert_eq!(
            normalize_pasted_text(input),
            "Uno dos tres cuatro cinco seis siete ocho nueve diez.\n\nSegundo párrafo."
        );
    }

    #[test]
    fn paste_can_prefix_insert_mode_before_payload() {
        let win32 = false;
        let prepared = normalize_pasted_text("texto");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(
            &encode_input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), win32)
                .expect("Esc debe codificarse"),
        );
        bytes.extend_from_slice(
            &encode_input(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), win32)
                .expect("i debe codificarse"),
        );
        bytes.extend_from_slice(&encode_paste(&prepared, win32));
        assert!(bytes.starts_with(b"\x1bi\x1b[200~"));
    }

    #[test]
    fn multiline_paste_preserves_paragraph_breaks_in_vt_mode() {
        let encoded = encode_paste("uno\r\ndos\rtres\ncuatro", false);
        assert!(encoded.starts_with(b"\x1b[200~"));
        assert!(encoded.ends_with(b"\x1b[201~"));
        let payload = &encoded[6..encoded.len() - 6];
        assert_eq!(payload, b"uno\ndos\ntres\ncuatro");
    }

    #[test]
    fn multiline_paste_turns_line_breaks_into_enter_in_win32_mode() {
        let encoded = String::from_utf8(encode_paste("uno\r\ndos\ntres", true))
            .expect("la entrada Win32 debe quedar codificada como ASCII");
        let enter_down = ";13;1;0;1_";
        assert_eq!(encoded.matches(enter_down).count(), 2);
    }

    #[test]
    fn vt_ctrl_word_navigation_preserves_control_modifier() {
        let ctrl = KeyModifiers::CONTROL;

        assert_eq!(
            encode_key(KeyEvent::new(KeyCode::Left, ctrl)).as_deref(),
            Some(b"\x1b[1;5D".as_slice())
        );
        assert_eq!(
            encode_key(KeyEvent::new(KeyCode::Right, ctrl)).as_deref(),
            Some(b"\x1b[1;5C".as_slice())
        );
        assert_eq!(
            encode_key(KeyEvent::new(KeyCode::Delete, ctrl)).as_deref(),
            Some(b"\x1b[3;5~".as_slice())
        );
        assert_eq!(
            encode_key(KeyEvent::new(KeyCode::F(16), KeyModifiers::NONE)).as_deref(),
            Some(b"\x1b[29~".as_slice())
        );
    }

    #[test]
    fn zen_theme_is_valid_toml_and_keeps_highlighter_background() {
        let root = std::env::temp_dir().join(format!("helix-sst-theme-{}", std::process::id()));
        let _ = fs::create_dir_all(&root);
        let output = root.join("helix-sst-zen.toml");

        write_zen_theme(&output).expect("el tema debe generarse");
        let raw = fs::read_to_string(&output).expect("el tema debe leerse");
        let parsed: toml::Value = toml::from_str(&raw).expect("el tema debe ser TOML válido");

        assert_eq!(
            parsed.get("inherits").and_then(toml::Value::as_str),
            Some("gruvbox")
        );
        assert_eq!(
            parsed
                .get("macro")
                .and_then(toml::Value::as_table)
                .and_then(|style| style.get("bg"))
                .and_then(toml::Value::as_str),
            Some("#fabd2f")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn editor_config_keeps_windows_word_shortcuts_without_forced_wrap() {
        let root =
            std::env::temp_dir().join(format!("helix-sst-editor-config-{}", std::process::id()));
        let _ = fs::create_dir_all(&root);

        let output = root.join("config.toml");
        let launcher = root.join("helix-sst-zen.exe");
        write_editor_config(&output, &launcher).expect("config.toml debe generarse");

        let raw = fs::read_to_string(&output).expect("config.toml debe leerse");
        let parsed: toml::Value = toml::from_str(&raw).expect("config.toml debe ser TOML válido");

        let editor_table = parsed
            .get("editor")
            .and_then(toml::Value::as_table)
            .expect("debe existir [editor]");
        assert!(
            editor_table.get("text-width").is_none(),
            "Zenmode no debe imponer un ancho lógico de 88 columnas"
        );
        assert!(
            editor_table.get("soft-wrap").is_none(),
            "Zenmode no debe partir visualmente cada párrafo como si fuera una sola línea"
        );
        assert_eq!(
            editor_table
                .get("line-number")
                .and_then(toml::Value::as_str),
            Some("absolute")
        );

        let keys = parsed
            .get("keys")
            .and_then(toml::Value::as_table)
            .expect("debe existir [keys]");

        let normal = keys
            .get("normal")
            .and_then(toml::Value::as_table)
            .expect("debe existir [keys.normal]");
        assert_eq!(
            normal.get("C-left").and_then(toml::Value::as_str),
            Some("move_prev_word_start")
        );
        assert_eq!(
            normal.get("C-right").and_then(toml::Value::as_str),
            Some("move_next_word_start")
        );
        assert_eq!(
            normal.get("F13").and_then(toml::Value::as_str),
            Some("move_prev_word_start")
        );
        assert_eq!(
            normal.get("F14").and_then(toml::Value::as_str),
            Some("move_next_word_start")
        );
        assert_eq!(
            normal.get("C-z").and_then(toml::Value::as_str),
            Some("undo")
        );
        assert_eq!(
            normal.get("C-y").and_then(toml::Value::as_str),
            Some("redo")
        );

        let insert = keys
            .get("insert")
            .and_then(toml::Value::as_table)
            .expect("debe existir [keys.insert]");
        for (key, command) in [
            ("C-left", "move_prev_word_start"),
            ("C-right", "move_next_word_start"),
            ("C-backspace", "delete_word_backward"),
            ("C-del", "delete_word_forward"),
            ("F13", "move_prev_word_start"),
            ("F14", "move_next_word_start"),
            ("F15", "delete_word_backward"),
            ("F16", "delete_word_forward"),
        ] {
            assert_eq!(
                insert.get(key).and_then(toml::Value::as_str),
                Some(command),
                "binding incorrecto para {key}"
            );
        }

        let _ = fs::remove_file(output);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extensionless_language_config_attaches_spell_server_without_forced_wrap() {
        let root =
            std::env::temp_dir().join(format!("helix-sst-language-config-{}", std::process::id()));
        let _ = fs::create_dir_all(&root);

        let output = root.join("languages.toml");
        let launcher = root.join("helix-sst-zen.exe");
        let dictionary = root.join(".spell-user");
        let current_file = root.join("Pensamentao");

        write_language_config(&output, &launcher, &dictionary, &current_file)
            .expect("languages.toml debe generarse");

        let raw = fs::read_to_string(&output).expect("languages.toml debe leerse");
        let parsed: toml::Value =
            toml::from_str(&raw).expect("languages.toml debe ser TOML válido");
        let languages = parsed
            .get("language")
            .and_then(toml::Value::as_array)
            .expect("debe existir [[language]]");

        let text = languages
            .iter()
            .find(|language| language.get("name").and_then(toml::Value::as_str) == Some("prose"))
            .expect("debe existir el lenguaje prose");

        assert!(text.get("text-width").is_none());
        assert!(text.get("soft-wrap").is_none());

        let servers = text
            .get("language-servers")
            .and_then(toml::Value::as_array)
            .expect("prose debe declarar language-servers");
        assert!(
            servers
                .iter()
                .any(|server| server.as_str() == Some("helix-sst-spell")),
            "prose debe usar helix-sst-spell"
        );

        let file_types = text
            .get("file-types")
            .and_then(toml::Value::as_array)
            .expect("prose debe declarar file-types");
        assert!(
            file_types
                .iter()
                .any(|entry| entry.as_str() == Some("Pensamentao")),
            "el archivo sin extensión debe registrarse por nombre"
        );
        assert!(
            file_types.iter().any(|entry| {
                entry
                    .get("glob")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|glob| glob.ends_with("/Pensamentao"))
            }),
            "el archivo sin extensión debe registrarse también por glob absoluto"
        );

        let _ = fs::remove_file(output);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn win32_ctrl_special_keys_keep_control_state() {
        let ctrl = KeyModifiers::CONTROL;

        let left = String::from_utf8(
            encode_input(KeyEvent::new(KeyCode::Left, ctrl), true)
                .expect("Ctrl+Left debe codificarse"),
        )
        .expect("la secuencia Win32 debe ser ASCII");
        assert!(left.contains(";1;264;1_"));
        assert!(left.contains(";0;264;1_"));

        let backspace = String::from_utf8(
            encode_input(KeyEvent::new(KeyCode::Backspace, ctrl), true)
                .expect("Ctrl+Backspace debe codificarse"),
        )
        .expect("la secuencia Win32 debe ser ASCII");
        assert!(backspace.contains(";1;8;1_"));
        assert!(backspace.contains(";0;8;1_"));

        let delete = String::from_utf8(
            encode_input(KeyEvent::new(KeyCode::Delete, ctrl), true)
                .expect("Ctrl+Delete debe codificarse"),
        )
        .expect("la secuencia Win32 debe ser ASCII");
        assert!(delete.contains(";1;264;1_"));
        assert!(delete.contains(";0;264;1_"));
    }
}
