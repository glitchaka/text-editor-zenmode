$ErrorActionPreference = 'Stop'

function Read-Lf([string]$Path) {
  [System.IO.File]::ReadAllText($Path).Replace("`r`n", "`n")
}
function Write-Utf8([string]$Path, [string]$Text) {
  [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}
function Replace-Once([string]$Text, [string]$Old, [string]$New, [string]$Label) {
  $i = $Text.IndexOf($Old)
  if ($i -lt 0) { throw "No se encontró bloque: $Label" }
  return $Text.Substring(0, $i) + $New + $Text.Substring($i + $Old.Length)
}

$e = Read-Lf 'src/editor.rs'
if (-not $e.Contains('source_stamp: Option<FileStamp>')) {
  $e = Replace-Once $e @'
struct NativeBuffer {
    source: PathBuf,
    edit: PathBuf,
    last_stamp: Option<FileStamp>,
    last_body: String,
}
'@ @'
struct NativeBuffer {
    source: PathBuf,
    edit: PathBuf,
    last_stamp: Option<FileStamp>,
    source_stamp: Option<FileStamp>,
    last_body: String,
}
'@ 'NativeBuffer.source_stamp'
}
if (-not $e.Contains('source_stamp: file_stamp(source),')) {
  $e = Replace-Once $e @'
    Ok(NativeBuffer {
        source: source.to_path_buf(),
        edit,
        last_stamp,
        last_body,
    })
'@ @'
    Ok(NativeBuffer {
        source: source.to_path_buf(),
        edit,
        last_stamp,
        source_stamp: file_stamp(source),
        last_body,
    })
'@ 'NativeBuffer init'
}

$syncStart = $e.IndexOf('    fn sync_native_inner(&mut self, force: bool) -> Result<bool> {')
$syncEnd = $e.IndexOf('    pub fn paste(', $syncStart)
if ($syncStart -lt 0 -or $syncEnd -lt 0) { throw 'No se encontró sync_native_inner' }
$newSync = @'
    fn sync_native_inner(&mut self, force: bool) -> Result<bool> {
        let Some(native) = self.native.as_ref() else {
            return Ok(false);
        };

        let edit_stamp = file_stamp(&native.edit);
        let source_stamp = file_stamp(&native.source);
        let edit_changed = edit_stamp != native.last_stamp;
        let source_changed = source_stamp != native.source_stamp;
        if !force && !edit_changed && !source_changed {
            return Ok(false);
        }

        let source = native.source.clone();
        let edit = native.edit.clone();
        let previous_body = native.last_body.clone();
        let body = fs::read_to_string(&edit)
            .with_context(|| format!("No se pudo leer el cuerpo editable {}", edit.display()))?;
        let mut document = document::read(&source)?;
        let body_changed = document.body != body;
        let formatting_only_change = source_changed && !edit_changed && !body_changed;

        if body_changed {
            document.formatting =
                document::remap_formatting(&document.formatting, &previous_body, &body);
            document.body = body.clone();
            document::write(&source, &document)
                .with_context(|| format!("No se pudo guardar {}", source.display()))?;
        }

        let final_source_stamp = file_stamp(&source);
        if let Some(native) = self.native.as_mut() {
            native.last_stamp = edit_stamp;
            native.source_stamp = final_source_stamp;
            native.last_body = body;
        }

        if formatting_only_change {
            self.send_command(":lsp-restart")?;
        }

        Ok(body_changed || formatting_only_change)
    }

'@
$e = $e.Substring(0, $syncStart) + $newSync + $e.Substring($syncEnd)
Write-Utf8 'src/editor.rs' $e

$s = Read-Lf 'src/spell.rs'
$s = $s.Replace("    next_request_id: u64,`n", '')
$s = $s.Replace("            next_request_id: 1,`n", '')
$refreshPattern = '(?ms)\s*else if changed && command == Some\(FORMAT_COMMAND\) \{\s*let request_id = server\.next_request_id;\s*server\.next_request_id = server\.next_request_id\.saturating_add\(1\);\s*send_request\(\s*&mut output,\s*json!\(request_id\),\s*"workspace/semanticTokens/refresh",\s*Value::Null,\s*\)\?;\s*\}'
$s = [regex]::Replace($s, $refreshPattern, '')
$requestPattern = '(?ms)\nfn send_request\(output: &mut impl Write, id: Value, method: &str, params: Value\) -> Result<\(\)> \{.*?\n\}\n(?=fn send_notification)'
$s = [regex]::Replace($s, $requestPattern, "`n")
Write-Utf8 'src/spell.rs' $s

$a = Read-Lf 'src/app.rs'

if (-not $a.Contains('    pomodoro_requested: bool,')) {
  $a = Replace-Once $a "    pomodoro_offer: bool,`n" "    pomodoro_requested: bool,`n    pomodoro_offer: bool,`n" 'campo pomodoro_requested'
}
if (-not $a.Contains('            pomodoro_requested: false,')) {
  $a = Replace-Once $a "            pomodoro_offer: false,`n" "            pomodoro_requested: false,`n            pomodoro_offer: false,`n" 'init pomodoro_requested'
}

$oldOpen = @'
        self.reset_parser();
        self.chapter_switch_until = None;
        self.editor = Some(session);
        self.pomodoro_offer = true;
        self.pomodoro_active = false;
        self.pomodoro_break = false;
        self.pomodoro_deadline = None;
        self.command_capture = None;
        self.glyphs.clear();
        self.dirty = true;
        Ok(())
'@
$newOpen = @'
        self.reset_parser();
        self.chapter_switch_until = None;
        let start_pomodoro = self.pomodoro_requested;
        self.editor = Some(session);
        self.pomodoro_offer = false;
        self.pomodoro_active = false;
        self.pomodoro_break = false;
        self.pomodoro_deadline = None;
        self.command_capture = None;
        self.glyphs.clear();
        self.dirty = true;
        if start_pomodoro {
            self.start_pomodoro();
        }
        Ok(())
'@
if ($a.Contains($oldOpen)) {
  $a = Replace-Once $a $oldOpen $newOpen 'open_editor pomodoro launcher'
} elseif (-not $a.Contains('let start_pomodoro = self.pomodoro_requested;')) {
  throw 'No se encontró bloque open_editor esperado'
}

if (-not $a.Contains('Pomodoro 25/5 seleccionado para la próxima apertura.')) {
  $marker = @'
            KeyCode::Char('z') | KeyCode::Char('Z') => {
'@
  $insert = @'
            KeyCode::F(4) | KeyCode::Char('p') | KeyCode::Char('P') => {
                self.pomodoro_requested = !self.pomodoro_requested;
                self.launcher.message = Some(if self.pomodoro_requested {
                    "Pomodoro 25/5 seleccionado para la próxima apertura.".into()
                } else {
                    "Pomodoro desactivado para la próxima apertura.".into()
                });
            }
            KeyCode::Char('z') | KeyCode::Char('Z') => {
'@
  $a = Replace-Once $a $marker $insert 'tecla Pomodoro launcher'
}

if (-not $a.Contains('POMODORO 25/5')) {
  $archiveHeader = @'
        push_line(
            &mut out,
            &format!("\x1b[38;5;244m├{}┤\x1b[0m", "─".repeat(inner_width)),
        );
        push_line(
            &mut out,
            &framed_left("\x1b[1;38;5;222mARCHIVOS\x1b[0m", inner_width, "38;5;244"),
        );
'@
  $pomodoroRow = @'
        let pomodoro_state = if self.pomodoro_requested {
            "\x1b[1;38;5;222m●\x1b[0m  POMODORO 25/5  \x1b[38;5;250m(F4 o P para cambiar)\x1b[0m"
        } else {
            "\x1b[38;5;244m○\x1b[0m  POMODORO apagado  \x1b[38;5;250m(F4 o P para activar)\x1b[0m"
        };
        push_line(
            &mut out,
            &framed_left(pomodoro_state, inner_width, "38;5;244"),
        );

        push_line(
            &mut out,
            &format!("\x1b[38;5;244m├{}┤\x1b[0m", "─".repeat(inner_width)),
        );
        push_line(
            &mut out,
            &framed_left("\x1b[1;38;5;222mARCHIVOS\x1b[0m", inner_width, "38;5;244"),
        );
'@
  $a = Replace-Once $a $archiveHeader $pomodoroRow 'fila Pomodoro launcher'
}

$a = $a.Replace(
  '↑↓ seleccionar  Enter abrir  N nuevo  E exportar  Z modo  Backspace subir  R refrescar',
  '↑↓ seleccionar  Enter abrir  N nuevo  E exportar  F4/P Pomodoro  Z modo  Backspace subir  R refrescar'
)

foreach ($x in @(178, 210, 242, 276, 314, 352)) {
  $pattern = '(?ms)(Rectangle\s*\{\s*)visible: root\.editor-active && island\.width >= 840px;(\s*x:\s*' + $x + 'px;)'
  $replaced = [regex]::Replace($a, $pattern, '${1}visible: false;${2}', 1)
  if ($replaced -eq $a) { throw "No se pudo ocultar control de isla x=$x" }
  $a = $replaced
}

Write-Utf8 'src/app.rs' $a

$c = Read-Lf 'Cargo.toml'
$c = $c.Replace('version = "0.2.11"', 'version = "0.2.12"')
Write-Utf8 'Cargo.toml' $c
