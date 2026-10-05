from pathlib import Path

p = Path('.github/scripts/implement_pomodoro.py')
s = p.read_text(encoding='utf-8')
old = '''s = replace_once(s, initial_set, initial_new, "initial pomodoro UI")
# Replace second occurrence in timer by performing once more.
s = replace_once(s, initial_set, initial_new, "timer pomodoro UI")
'''
new = '''if initial_set not in s:
    raise SystemExit("No se encontró bloque de UI Pomodoro")
s = s.replace(initial_set, initial_new)
'''
if old not in s:
    raise SystemExit('No se encontró el bloque a corregir en implement_pomodoro.py')
p.write_text(s.replace(old, new, 1), encoding='utf-8')
