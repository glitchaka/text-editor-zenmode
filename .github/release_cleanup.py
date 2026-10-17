from pathlib import Path
import os, re, subprocess, time

REPO = os.environ["REPO"]

def sh(*args, check=True):
    p = subprocess.run(args, text=True, capture_output=True)
    if check and p.returncode != 0:
        raise SystemExit(p.stderr or p.stdout)
    return p

def out(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.DEVNULL).strip()

changelog = Path("CHANGELOG.md").read_text(encoding="utf-8")
parts = re.split(r'(?m)^## v([0-9]+\.[0-9]+\.[0-9]+)(?: — [^\n]+)?\n', changelog)
notes = {}
order = []
for i in range(1, len(parts), 2):
    version = parts[i]
    bullets = [x.strip() for x in parts[i + 1].splitlines() if x.strip().startswith("- ")]
    original = bullets[:]
    cleaned = []
    noise = ("workflow", "[skip ci]", "rustfmt", "clippy", "script temporal", "parche temporal", "ci temporal", "one-shot", "temporary")
    for bullet in bullets:
        text = bullet[2:].strip()
        low = text.lower()
        if low.startswith(("chore:", "test:", "style:")):
            continue
        if any(token in low for token in noise):
            continue
        if low.startswith(("preparar ", "ejecutar ", "retirar ")):
            continue
        cleaned.append(bullet)
    if not cleaned:
        cleaned = original[:4] or ["- Actualización de Zen Editor."]
    notes[version] = cleaned
    order.append(version)

commits = out("git", "rev-list", "--reverse", "HEAD").splitlines()
transitions = []
last = None
for sha in commits:
    p = subprocess.run(["git", "show", f"{sha}:Cargo.toml"], text=True, capture_output=True)
    if p.returncode != 0:
        continue
    m = re.search(r'(?m)^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"', p.stdout)
    if not m:
        continue
    v = m.group(1)
    if v != last:
        transitions.append((v, sha))
        last = v
root = out("git", "rev-list", "--max-parents=0", "HEAD").splitlines()[0]
first = {}
for v, sha in transitions:
    first.setdefault(v, sha)
first.setdefault("0.0.1", root)
chronological = list(reversed(order))

def target_for(version):
    idx = chronological.index(version)
    for later in chronological[idx + 1:]:
        if later in first:
            return out("git", "rev-parse", f"{first[later]}^")
    return out("git", "rev-parse", "HEAD")

def retry(args):
    for attempt in range(6):
        p = sh(*args, check=False)
        if p.returncode == 0:
            return
        if attempt == 5:
            raise SystemExit(p.stderr or p.stdout)
        time.sleep(5 * (attempt + 1))

for version in chronological:
    tag = f"v{version}"
    body = f"## Zen Editor {tag}\n\n### Cambios\n" + "\n".join(notes[version]) + "\n"
    note_path = Path(f"/tmp/{tag}.md")
    note_path.write_text(body, encoding="utf-8")
    exists = sh("gh", "release", "view", tag, "--repo", REPO, check=False).returncode == 0
    if exists:
        retry(["gh", "release", "edit", tag, "--repo", REPO, "--title", f"Zen Editor {tag}", "--notes-file", str(note_path)])
    else:
        retry(["gh", "release", "create", tag, "--repo", REPO, "--target", target_for(version), "--title", f"Zen Editor {tag}", "--notes-file", str(note_path)])
    time.sleep(2)
