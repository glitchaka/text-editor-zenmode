from pathlib import Path
import re


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"No se encontró ancla: {label}")
    return text.replace(old, new, 1)


app_path = Path("src/app.rs")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    "            x: root.width - 224px;\n            y: root.height - 10px;\n            width: 224px;",
    "            x: root.width - 260px;\n            y: root.height - 10px;\n            width: 260px;",
    "bottom reveal",
)
app = replace_once(
    app,
    "            width: 196px;\n            height: 34px;",
    "            width: 232px;\n            height: 34px;",
    "island width",
)
old_close = '''            Rectangle {
                x: 157px; y: 1px; width: 38px; height: 32px;
                border-radius: 10px;
                background: close-touch.pressed
                    ? rgb(62, 23, 36)
                    : close-touch.has-hover ? rgb(48, 18, 28) : transparent;
                Path {
                    x: 10px; y: 8px; width: 18px; height: 18px;
                    commands: "M 9 1 L 9 8 M 3.3 3.7 A 7 7 0 1 0 14.7 3.7";
                    stroke: close-touch.has-hover ? #ff5d78 : #ff9fbd;
                    stroke-width: 2px; stroke-line-cap: round;
                }
                close-touch := TouchArea { mouse-cursor: pointer; clicked => { root.close-window(); } }
            }
'''
new_close = '''            Rectangle {
                x: 157px; y: 3px; width: 32px; height: 28px;
                border-radius: 9px;
                background: fullscreen-touch.pressed
                    ? #29384b
                    : fullscreen-touch.has-hover ? #172334 : transparent;
                Text {
                    width: 100%; height: 100%; text: "⛶"; color: #dfe8ef;
                    font-family: "Segoe UI Variable"; font-size: 15px;
                    horizontal-alignment: center; vertical-alignment: center;
                }
                fullscreen-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.toggle-zen(); terminal-focus.focus(); }
                }
            }

            Rectangle {
                x: 193px; y: 1px; width: 38px; height: 32px;
                border-radius: 10px;
                background: close-touch.pressed
                    ? rgb(62, 23, 36)
                    : close-touch.has-hover ? rgb(48, 18, 28) : transparent;
                Path {
                    x: 10px; y: 8px; width: 18px; height: 18px;
                    commands: "M 9 1 L 9 8 M 3.3 3.7 A 7 7 0 1 0 14.7 3.7";
                    stroke: close-touch.has-hover ? #ff5d78 : #ff9fbd;
                    stroke-width: 2px; stroke-line-cap: round;
                }
                close-touch := TouchArea { mouse-cursor: pointer; clicked => { root.close-window(); } }
            }
'''
app = replace_once(app, old_close, new_close, "fullscreen button")
f4 = '''        if key.code == KeyCode::F(4) && key.modifiers.is_empty() {
            self.start_pomodoro();
            return;
        }
'''
app = replace_once(
    app,
    f4,
    '''        if key.code == KeyCode::F(11) && key.modifiers.is_empty() {
            self.toggle_editor_zen();
            return;
        }
''' + f4,
    "F11 editor",
)
z_block = '''            KeyCode::Char('z') | KeyCode::Char('Z') => {
                self.zen_requested = !self.zen_requested;
                self.launcher.message = Some(if self.zen_requested {
                    "Zenmode real seleccionado.".into()
                } else {
                    "Modo normal seleccionado.".into()
                });
            }
'''
app = replace_once(
    app,
    z_block,
    '''            KeyCode::F(11) => {
                self.zen_requested = !self.zen_requested;
                self.launcher.message = Some(if self.zen_requested {
                    "Pantalla completa seleccionada.".into()
                } else {
                    "Modo ventana seleccionado.".into()
                });
            }
''' + z_block,
    "F11 launcher",
)
app_path.write_text(app, encoding="utf-8")

spell_path = Path("src/spell.rs")
spell = spell_path.read_text(encoding="utf-8")
spell = replace_once(
    spell,
    '''            let mut push_format = |title: &str, action: &str, value: &str| {
                actions.push(json!({
''',
    '''            let mut format_actions = Vec::new();
            let mut push_format = |title: &str, action: &str, value: &str| {
                format_actions.push(json!({
''',
    "format action priority",
)
spell = replace_once(
    spell,
    '''            push_format("Alineación · Justificada", "alignment", "justify");
        }
        actions
''',
    '''            push_format("Alineación · Justificada", "alignment", "justify");
            format_actions.extend(actions);
            actions = format_actions;
        }
        actions
''',
    "format action merge",
)
spell_path.write_text(spell, encoding="utf-8")

editor_path = Path("src/editor.rs")
editor = editor_path.read_text(encoding="utf-8")
editor = editor.replace(
    'normal = "NORMAL · i: escribir · F2: acciones · F4: Pomodoro"',
    'normal = "NORMAL · i: escribir · F2: acciones · F4: Pomodoro · F11: pantalla completa"',
)
editor = editor.replace(
    'insert = "INSERTAR · Alt+d: — · F2: acciones · F4: Pomodoro · Esc: comandos"',
    'insert = "INSERTAR · Alt+d: — · F2: acciones · F4: Pomodoro · F11: pantalla completa · Esc: comandos"',
)
editor_path.write_text(editor, encoding="utf-8")

cargo_path = Path("Cargo.toml")
cargo = cargo_path.read_text(encoding="utf-8")
cargo = replace_once(cargo, 'version = "0.2.17"', 'version = "0.2.18"', "Cargo version")
cargo_path.write_text(cargo, encoding="utf-8")

lock_path = Path("Cargo.lock")
lock = lock_path.read_text(encoding="utf-8")
lock, count = re.subn(
    r'(\[\[package\]\]\nname = "zen-editor"\nversion = ")([0-9]+\.[0-9]+\.[0-9]+)(")',
    r'\g<1>0.2.18\g<3>',
    lock,
    count=1,
)
if count != 1:
    raise SystemExit("No se pudo actualizar zen-editor en Cargo.lock")
lock_path.write_text(lock, encoding="utf-8")

readme_path = Path("README.md")
readme = readme_path.read_text(encoding="utf-8")
intro = '**Zen Editor** es un editor de escritura para Windows construido sobre **Helix 25.07.1**. Helix sigue siendo el motor real de edición dentro de una PTY; la interfaz exterior está escrita en Rust con Slint/FemtoVG y añade escritura sin distracciones, formato enriquecido no destructivo, corrector es-CL, proyectos, Pomodoro, importación y exportación.\n'
credits = intro + '\nZen Editor existe sobre el trabajo del proyecto **Helix** y de sus contribuidores. Gracias al equipo de Helix por construir y mantener el motor de edición que hace posible este proyecto. Zen Editor no es un producto oficial ni afiliado al proyecto Helix.\n\n## Créditos y licencia\n\n- **Helix Editor** — motor de edición integrado: https://github.com/helix-editor/helix\n- **Licencia de Helix:** Mozilla Public License 2.0.\n- La licencia completa está en [`LICENSE`](LICENSE) y la información de terceros en [`THIRD_PARTY.md`](THIRD_PARTY.md) y [`CREDITS.md`](CREDITS.md).\n'
readme = replace_once(readme, intro, credits, "README credits")
readme = readme.replace(
    'La isla inferior se mantiene compacta: **Pomodoro · Ω símbolos · cerrar**.',
    'La isla inferior se mantiene compacta: **Pomodoro · Ω símbolos · pantalla completa · cerrar**.',
)
readme = readme.replace('| `F4` | Pomodoro |', '| `F4` | Pomodoro |\n| `F11` | entrar/salir de pantalla completa |')
readme_path.write_text(readme, encoding="utf-8")

Path("THIRD_PARTY.md").write_text(
    '''# Componentes de terceros\n\n## Helix Editor\n\nZen Editor integra **Helix 25.07.1** como motor real de edición.\n\nProyecto original: https://github.com/helix-editor/helix\n\nHelix se distribuye bajo **Mozilla Public License 2.0 (MPL-2.0)**. El texto completo de la licencia se conserva en `LICENSE` y `licenses/HELIX-MPL-2.0.txt`.\n\n**Agradecimiento:** gracias al equipo y a todos los contribuidores de Helix por desarrollar y mantener el editor sobre el que se construye Zen Editor. Zen Editor es un proyecto independiente y no oficial.\n\n## Diccionario ortográfico español de Chile\n\nEl diccionario Hunspell es-CL se obtiene desde `wooorm/dictionaries` en la revisión fijada en `build.rs`. Su licencia se integra con los recursos generados del programa.\n''',
    encoding="utf-8",
)
Path("CREDITS.md").write_text(
    '''# Créditos\n\n## Helix\n\nZen Editor usa **Helix Editor 25.07.1** como motor de edición.\n\nGracias al equipo de Helix y a sus contribuidores por su trabajo en un editor rápido, modal y extensible que sirve como base de Zen Editor.\n\n- Proyecto: https://github.com/helix-editor/helix\n- Licencia: Mozilla Public License 2.0 (MPL-2.0)\n- Zen Editor es un proyecto independiente y no está afiliado oficialmente con Helix.\n''',
    encoding="utf-8",
)
Path("LICENSE").write_text(
    Path("licenses/HELIX-MPL-2.0.txt").read_text(encoding="utf-8"),
    encoding="utf-8",
)

changelog_path = Path("CHANGELOG.md")
changelog = changelog_path.read_text(encoding="utf-8")
changelog = changelog.replace(
    'Historial reconstruido desde el historial Git del proyecto. Cada versión corresponde al estado final del código antes del siguiente cambio de versión.',
    'Cambios por versión de Zen Editor.',
)
if '## v0.2.18' not in changelog:
    marker = 'Cambios por versión de Zen Editor.\n\n'
    entry = '''## v0.2.18 — 2026-10-17\n\n- Restaurar control visible y atajo F11 para pantalla completa.\n- Corregir el orden del menú F2 para mostrar primero formato y alineación cuando hay selección.\n- Añadir licencia visible, créditos y agradecimiento al proyecto Helix.\n- Limpiar las notas de las releases para mostrar solo información útil de cada versión.\n\n'''
    changelog = replace_once(changelog, marker, marker + entry, "CHANGELOG 0.2.18")
changelog_path.write_text(changelog, encoding="utf-8")
