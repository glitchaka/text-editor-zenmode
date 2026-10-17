from pathlib import Path

p = Path('src/world.rs')
text = p.read_text(encoding='utf-8')
blocks = [
'''    pub fn rename_record(&mut self, section: WorldSection, index: usize, title: &str) -> Result<bool> {
        if !section.is_record_list() || title.trim().is_empty() {
            return Ok(false);
        }
        let Some(record) = self.array_mut(section).get_mut(index).and_then(Value::as_object_mut) else {
            return Ok(false);
        };
        let key = if record.contains_key("name") { "name" } else { "title" };
        record.insert(key.into(), json!(title.trim()));
        if section == WorldSection::Scenes {
            self.sync_scene_document(index)?;
        }
        self.touch();
        self.save()?;
        Ok(true)
    }

''',
'''    pub fn set_profile_field(&mut self, key: &str, value: &str) -> Result<()> {
        if let Some(profile) = self.root.get_mut("profile").and_then(Value::as_object_mut) {
            profile.insert(key.to_owned(), json!(value));
        }
        self.touch();
        self.save()
    }

'''
]
for block in blocks:
    if block not in text:
        raise SystemExit('expected dead-code block not found')
    text = text.replace(block, '', 1)
p.write_text(text, encoding='utf-8')
