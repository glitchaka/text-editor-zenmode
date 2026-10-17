from pathlib import Path

p = Path('Cargo.toml')
s = p.read_text(encoding='utf-8')
if 'version = "0.2.19"' not in s:
    raise SystemExit('Cargo.toml version anchor missing')
p.write_text(s.replace('version = "0.2.19"', 'version = "0.2.20"', 1), encoding='utf-8')

p = Path('Cargo.lock')
s = p.read_text(encoding='utf-8')
old = 'name = "zen-editor"\nversion = "0.2.19"'
if old not in s:
    raise SystemExit('Cargo.lock version anchor missing')
p.write_text(s.replace(old, 'name = "zen-editor"\nversion = "0.2.20"', 1), encoding='utf-8')

p = Path('CHANGELOG.md')
s = p.read_text(encoding='utf-8')
entry = '''## v0.2.20 — 2026-10-17
- Eliminar restos visuales de los menús F2 mediante un repintado completo de Helix.
- Añadir creación de carpetas desde el launcher.
- Añadir agrupación de documentos por proyecto mediante un manifiesto de proyecto.
- Mantener el dock visible y compacto sin cubrir la línea de estado del editor.
- Mostrar contador de palabras en el dock.
- Cambiar F11/Zen a ventana maximizada sin ocultar la barra de tareas de Windows.

'''
if '## v0.2.20' not in s:
    marker = '## v0.2.19'
    if marker not in s:
        raise SystemExit('CHANGELOG version marker missing')
    s = s.replace(marker, entry + marker, 1)
p.write_text(s, encoding='utf-8')
