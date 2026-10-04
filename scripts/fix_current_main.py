from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly 1 match, found {count}")
    return text.replace(old, new, 1)


# src/app.rs: keep the launcher general-purpose and remove dead page-preview residue.
app_path = Path("src/app.rs")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    "const PAGE_PX_PER_MM: f32 = 3.6;\n",
    "",
    "remove unused PAGE_PX_PER_MM",
)
app = replace_once(
    app,
    "            None => (library.documents.clone(), None),\n",
    "            None => (current.clone(), None),\n",
    "start launcher in current working directory",
)
app = replace_once(
    app,
    '''            KeyCode::Backspace => {\n                if self.launcher.cwd != self.library.documents\n                    && let Some(parent) = self.launcher.cwd.parent().map(Path::to_path_buf)\n                {\n                    self.launcher.cwd = parent;\n                    self.launcher.selected = 0;\n                    self.launcher.message = None;\n                    self.launcher.refresh();\n                }\n            }\n''',
    '''            KeyCode::Backspace => {\n                if let Some(parent) = self.launcher.cwd.parent().map(Path::to_path_buf) {\n                    self.launcher.cwd = parent;\n                    self.launcher.selected = 0;\n                    self.launcher.message = None;\n                    self.launcher.refresh();\n                }\n            }\n''',
    "allow launcher navigation above HSST library",
)
app = replace_once(
    app,
    '''            Ok(read_dir) => read_dir\n                .flatten()\n                .filter_map(|entry| {\n                    let path = entry.path();\n                    let directory = path.is_dir();\n                    let name = entry.file_name().to_string_lossy().into_owned();\n                    let metadata = document::read_metadata(&path);\n                    Some(Entry {\n                        path,\n                        name,\n                        directory,\n                        metadata,\n                    })\n                })\n                .collect::<Vec<_>>(),\n''',
    '''            Ok(read_dir) => read_dir\n                .flatten()\n                .map(|entry| {\n                    let path = entry.path();\n                    let directory = path.is_dir();\n                    let name = entry.file_name().to_string_lossy().into_owned();\n                    let metadata = document::read_metadata(&path);\n                    Entry {\n                        path,\n                        name,\n                        directory,\n                        metadata,\n                    }\n                })\n                .collect::<Vec<_>>(),\n''',
    "replace unnecessary filter_map",
)
app_path.write_text(app, encoding="utf-8")


# src/document.rs: legacy text serializer is kept only for legacy round-trip tests.
doc_path = Path("src/document.rs")
doc = doc_path.read_text(encoding="utf-8")
doc = replace_once(
    doc,
    "pub fn serialize(document: &HsstDocument) -> String {\n",
    "#[cfg(test)]\npub fn serialize(document: &HsstDocument) -> String {\n",
    "gate legacy serialize to tests",
)
doc = replace_once(
    doc,
    "fn toml_escape(value: &str) -> String {\n",
    "#[cfg(test)]\nfn toml_escape(value: &str) -> String {\n",
    "gate legacy toml_escape to tests",
)
doc_path.write_text(doc, encoding="utf-8")


# src/format.rs: markup_spans was part of the old visible-markup renderer and is no longer used.
format_path = Path("src/format.rs")
fmt = format_path.read_text(encoding="utf-8")
start_marker = "pub fn markup_spans(line: &str) -> Vec<(usize, usize)> {\n"
end_marker = "\n#[cfg(test)]\nmod tests {\n"
start = fmt.find(start_marker)
end = fmt.find(end_marker, start)
if start < 0 or end < 0:
    raise SystemExit("remove obsolete markup_spans helpers: block not found")
fmt = fmt[:start] + fmt[end + 1 :]
format_path.write_text(fmt, encoding="utf-8")

print("fixed current main: launcher general files + warning cleanup")
