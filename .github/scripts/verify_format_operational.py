from pathlib import Path

editor_path = Path("src/editor.rs")
editor = editor_path.read_text(encoding="utf-8")

old = "    let launcher = std::env::current_exe()?;"
new = '''    let launcher = std::env::var_os("HELIX_SST_TEST_LAUNCHER")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_exe()?);'''
if old not in editor:
    raise SystemExit("No se encontro launcher en ensure_installed")
editor = editor.replace(old, new, 1)

marker = "live_hsst_format_reaches_terminal_attributes"
if marker not in editor:
    editor += r'''

#[cfg(all(test, windows))]
mod live_format_operational_tests {
    use super::*;
    use std::{thread, time::Duration};

    #[derive(Clone, Copy)]
    enum ExpectedStyle {
        Bold,
        Italic,
        Underline,
        ForegroundRed,
    }

    fn word_has_style(session: &EditorSession, word: &str, expected: ExpectedStyle) -> bool {
        let parser = session.protocol.lock().unwrap_or_else(|e| e.into_inner());
        let screen = parser.screen();
        let (rows, cols) = screen.size();

        for row in 0..rows {
            let mut text = String::with_capacity(cols as usize);
            let mut column_map = Vec::<u16>::with_capacity(cols as usize);
            for col in 0..cols {
                let Some(cell) = screen.cell(row, col) else {
                    continue;
                };
                if cell.is_wide_continuation() {
                    continue;
                }
                let contents = cell.contents();
                if contents.is_empty() {
                    text.push(' ');
                    column_map.push(col);
                } else {
                    for ch in contents.chars() {
                        text.push(ch);
                        column_map.push(col);
                    }
                }
            }

            let Some(byte_start) = text.find(word) else {
                continue;
            };
            let char_start = text[..byte_start].chars().count();
            let char_end = char_start + word.chars().count();
            if char_end > column_map.len() {
                continue;
            }

            return (char_start..char_end).all(|index| {
                let Some(cell) = screen.cell(row, column_map[index]) else {
                    return false;
                };
                match expected {
                    ExpectedStyle::Bold => cell.bold(),
                    ExpectedStyle::Italic => cell.italic(),
                    ExpectedStyle::Underline => cell.underline(),
                    ExpectedStyle::ForegroundRed => {
                        cell.fgcolor() == vt100::Color::Rgb(0xfb, 0x49, 0x34)
                    }
                }
            });
        }
        false
    }

    #[test]
    fn live_refresh_after_f2_metadata_change_reaches_terminal() {
        let root = std::env::temp_dir().join(format!(
            "helix-sst-live-refresh-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("crear directorio temporal");
        let path = root.join("dynamic-format.hsst");

        let document = document::HsstDocument {
            metadata: document::DocumentMetadata::new("dynamic-format"),
            body: "DINAMICO".to_owned(),
            formatting: serde_json::json!({
                "version": 1,
                "unit": "utf8-byte",
                "runs": []
            }),
        };
        document::write(&path, &document).expect("crear HSST dinamico");

        let session = EditorSession::start(&path, 100, 28).expect("arrancar Helix real en PTY");
        let startup_deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < startup_deadline {
            while let Some(result) = session.try_output() {
                result.expect("salida de Helix valida");
            }
            if session
                .protocol
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .screen()
                .contents()
                .contains("DINAMICO")
            {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        assert!(
            document::apply_format_lsp_range(
                &path,
                "DINAMICO",
                0,
                0,
                0,
                8,
                "font-color",
                "red",
            )
            .expect("aplicar color rojo"),
            "el cambio de metadatos de formato no se aplico"
        );
        session
            .refresh_after_formatting()
            .expect("refrescar formato tras F2");

        let deadline = Instant::now() + Duration::from_secs(20);
        let mut red = false;
        while Instant::now() < deadline {
            while let Some(result) = session.try_output() {
                result.expect("salida de Helix valida");
            }
            red = word_has_style(&session, "DINAMICO", ExpectedStyle::ForegroundRed);
            if red {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        let _ = session.send_command(":quit!");
        assert!(
            red,
            "el refresco tras el cambio F2 no llevo el color rojo al terminal"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn live_hsst_format_reaches_terminal_attributes() {
        let root = std::env::temp_dir().join(format!(
            "helix-sst-live-format-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("crear directorio temporal");
        let path = root.join("visual-format.hsst");

        let document = document::HsstDocument {
            metadata: document::DocumentMetadata::new("visual-format"),
            body: "NEGRITA CURSIVA SUBRAYADO".to_owned(),
            formatting: serde_json::json!({
                "version": 1,
                "unit": "utf8-byte",
                "runs": [
                    {"start": 0, "end": 7, "bold": true},
                    {"start": 8, "end": 15, "italic": true},
                    {"start": 16, "end": 25, "underline": true}
                ]
            }),
        };
        document::write(&path, &document).expect("crear HSST de prueba");

        let session = EditorSession::start(&path, 100, 28).expect("arrancar Helix real en PTY");
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut bold = false;
        let mut italic = false;
        let mut underline = false;

        while Instant::now() < deadline {
            while let Some(result) = session.try_output() {
                result.expect("salida de Helix valida");
            }
            bold = word_has_style(&session, "NEGRITA", ExpectedStyle::Bold);
            italic = word_has_style(&session, "CURSIVA", ExpectedStyle::Italic);
            underline = word_has_style(&session, "SUBRAYADO", ExpectedStyle::Underline);
            if bold && italic && underline {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        let _ = session.send_command(":quit!");
        assert!(bold, "Helix no entrego atributo bold para NEGRITA");
        assert!(italic, "Helix no entrego atributo italic para CURSIVA");
        assert!(underline, "Helix no entrego atributo underline para SUBRAYADO");
        let _ = fs::remove_dir_all(root);
    }
}
'''

editor_path.write_text(editor, encoding="utf-8", newline="\n")

integration = r'''use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use zip::{write::FileOptions, ZipArchive, ZipWriter};

fn create_hsst(path: &Path, body: &str) {
    let file = File::create(path).expect("crear HSST");
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(
        br#"{"format":2,"id":"format-operational","title":"format-operational","project":"","type":"document","chapter":null,"order":0,"language":"es-CL","status":"draft"}"#,
    )
    .unwrap();
    zip.start_file("content.txt", options).unwrap();
    zip.write_all(body.as_bytes()).unwrap();
    zip.start_file("formatting.json", options).unwrap();
    zip.write_all(br#"{"version":1,"unit":"utf8-byte","runs":[]}"#)
        .unwrap();
    zip.start_file("history.jsonl", options).unwrap();
    zip.finish().unwrap();
}

fn frame(value: &Value) -> Vec<u8> {
    let payload = serde_json::to_vec(value).unwrap();
    let mut out = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
    out.extend_from_slice(&payload);
    out
}

fn parse_frames(bytes: &[u8]) -> Vec<Value> {
    let mut output = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let Some(relative_end) = bytes[cursor..]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
        else {
            break;
        };
        let header_end = cursor + relative_end;
        let header = std::str::from_utf8(&bytes[cursor..header_end]).unwrap();
        let length = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length:"))
            .map(str::trim)
            .and_then(|value| value.parse::<usize>().ok())
            .expect("Content-Length");
        let body_start = header_end + 4;
        let body_end = body_start + length;
        output.push(serde_json::from_slice(&bytes[body_start..body_end]).unwrap());
        cursor = body_end;
    }
    output
}

fn response<'a>(messages: &'a [Value], id: u64) -> &'a Value {
    messages
        .iter()
        .find(|message| message.get("id").and_then(Value::as_u64) == Some(id))
        .unwrap_or_else(|| panic!("falta respuesta LSP id {id}: {messages:#?}"))
}

fn run_action(action: &str, menu_title: &str, token_type: u64, formatting_key: &str) {
    let exe = env!("CARGO_BIN_EXE_helix-sst-zen");
    let root = std::env::temp_dir().join(format!(
        "helix-sst-format-pipeline-{}-{action}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let source = root.join("document.hsst");
    let dictionary = root.join("user.dic");
    create_hsst(&source, "Alpha beta");

    let uri = "file:///format-operational.hsst";
    let range = json!({
        "start": {"line": 0, "character": 0},
        "end": {"line": 0, "character": 5}
    });

    let mut child = Command::new(exe)
        .arg("--helix-sst-spell")
        .arg(&dictionary)
        .arg(&root)
        .arg(&source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("arrancar LSP real");

    {
        let stdin = child.stdin.as_mut().unwrap();
        let messages = [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"prose","version":1,"text":"Alpha beta"}}}),
            json!({"jsonrpc":"2.0","id":2,"method":"textDocument/codeAction","params":{"textDocument":{"uri":uri},"range":range.clone(),"context":{"diagnostics":[]}}}),
            json!({"jsonrpc":"2.0","id":3,"method":"workspace/executeCommand","params":{"command":"helix-sst.formatSelection","arguments":[uri,range.clone(),action,""]}}),
            json!({"jsonrpc":"2.0","id":4,"method":"textDocument/semanticTokens/full","params":{"textDocument":{"uri":uri}}}),
            json!({"jsonrpc":"2.0","id":5,"method":"shutdown","params":null}),
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ];
        for message in &messages {
            stdin.write_all(&frame(message)).unwrap();
        }
    }
    drop(child.stdin.take());

    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                panic!("LSP no finalizo para {action}");
            }
            Err(error) => panic!("estado del LSP: {error}"),
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "LSP fallo para {action}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let messages = parse_frames(&output.stdout);

    let actions = response(&messages, 2)["result"].as_array().unwrap();
    assert!(
        actions.iter().any(|entry| {
            entry.get("title").and_then(Value::as_str) == Some(menu_title)
                && entry.pointer("/command/command").and_then(Value::as_str)
                    == Some("helix-sst.formatSelection")
        }),
        "F2 no expuso {menu_title}: {actions:#?}"
    );
    assert_eq!(response(&messages, 3)["result"], Value::Bool(true));

    let data = response(&messages, 4)
        .pointer("/result/data")
        .and_then(Value::as_array)
        .expect("semantic tokens");
    assert!(
        data.chunks(5).any(|chunk| {
            chunk.len() == 5
                && chunk[2].as_u64() == Some(5)
                && chunk[3].as_u64() == Some(token_type)
        }),
        "semantic token incorrecto para {action}: {data:#?}"
    );

    let file = File::open(&source).unwrap();
    let mut zip = ZipArchive::new(file).unwrap();
    let mut raw = String::new();
    zip.by_name("formatting.json")
        .unwrap()
        .read_to_string(&mut raw)
        .unwrap();
    let formatting: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(formatting["runs"][0][formatting_key], Value::Bool(true));
    assert_eq!(formatting["runs"][0]["start"].as_u64(), Some(0));
    assert_eq!(formatting["runs"][0]["end"].as_u64(), Some(5));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn f2_format_pipeline_is_operational_for_bold_italic_and_underline() {
    run_action("bold", "Formato · Negrita", 2, "bold");
    run_action("italic", "Formato · Cursiva", 4, "italic");
    run_action("underline", "Formato · Subrayado", 5, "underline");
}
'''
Path("tests/format_operational.rs").write_text(integration, encoding="utf-8", newline="\n")
