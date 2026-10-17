from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    if old not in text:
        raise SystemExit(f"missing pattern in {path}: {old[:180]!r}")
    p.write_text(text.replace(old, new, 1), encoding="utf-8")

replace_once(
    "src/app.rs",
    '''struct TerminalModel {\n''',
    '''type SemanticReference = (String, String);\ntype ReferencePicker = (String, Vec<SemanticReference>, usize);\n\nstruct TerminalModel {\n''',
)
replace_once(
    "src/app.rs",
    '''    reference_picker: Option<(String, Vec<(String, String)>, usize)>,\n''',
    '''    reference_picker: Option<ReferencePicker>,\n''',
)

replace_once(
    "src/world.rs",
    '''    pub fn summary(&self) -> WorldSummary {\n        let mut summary = WorldSummary::default();\n        summary.chapters = self.array(WorldSection::Manuscript).len();\n        summary.scenes = self.array(WorldSection::Scenes).len();\n        summary.characters = self.array(WorldSection::Characters).len();\n        summary.relationships = self.array(WorldSection::Relationships).len();\n        summary.theories = self.array(WorldSection::Theories).len();\n        summary.timeline = self.array(WorldSection::Timeline).len();\n        summary.world = self.array(WorldSection::World).len();\n        summary.maps = self.array(WorldSection::Maps).len();\n        summary.magic = self.array(WorldSection::Magic).len();\n        summary.references = self.array(WorldSection::References).len();\n        summary.notes = self.array(WorldSection::Notes).len();\n        summary.review = self.array(WorldSection::Review).len();\n        summary.media = self.array(WorldSection::Media).len();\n        summary.words = self\n            .array(WorldSection::Manuscript)\n            .iter()\n            .chain(self.array(WorldSection::Scenes).iter())\n            .filter_map(|row| row.get("words").and_then(Value::as_u64))\n            .map(|value| value as usize)\n            .sum();\n        summary\n    }\n''',
    '''    pub fn summary(&self) -> WorldSummary {\n        WorldSummary {\n            chapters: self.array(WorldSection::Manuscript).len(),\n            scenes: self.array(WorldSection::Scenes).len(),\n            characters: self.array(WorldSection::Characters).len(),\n            relationships: self.array(WorldSection::Relationships).len(),\n            theories: self.array(WorldSection::Theories).len(),\n            timeline: self.array(WorldSection::Timeline).len(),\n            world: self.array(WorldSection::World).len(),\n            maps: self.array(WorldSection::Maps).len(),\n            magic: self.array(WorldSection::Magic).len(),\n            references: self.array(WorldSection::References).len(),\n            notes: self.array(WorldSection::Notes).len(),\n            review: self.array(WorldSection::Review).len(),\n            media: self.array(WorldSection::Media).len(),\n            words: self\n                .array(WorldSection::Manuscript)\n                .iter()\n                .chain(self.array(WorldSection::Scenes).iter())\n                .filter_map(|row| row.get("words").and_then(Value::as_u64))\n                .map(|value| value as usize)\n                .sum(),\n        }\n    }\n''',
)
