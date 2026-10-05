use std::{fs, path::PathBuf};

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct LibraryPaths {
    pub documents: PathBuf,
    pub exports: PathBuf,
}

impl LibraryPaths {
    pub fn ensure() -> Result<Self> {
        let root = default_root();
        let documents = root.join("Documentos");
        let exports = root.join("exports");

        fs::create_dir_all(&documents)
            .with_context(|| format!("No se pudo crear {}", documents.display()))?;
        fs::create_dir_all(&exports)
            .with_context(|| format!("No se pudo crear {}", exports.display()))?;

        Ok(Self { documents, exports })
    }
}

pub fn default_root() -> PathBuf {
    if let Some(path) = std::env::var_os("HELIX_SST_LIBRARY")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return path;
    }

    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        return parent.to_path_buf();
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_has_stable_subdirectories() {
        let root = PathBuf::from("X:/example/Helix SST");
        let paths = LibraryPaths {
            documents: root.join("Documentos"),
            exports: root.join("exports"),
        };
        assert!(paths.documents.ends_with("Documentos"));
        assert!(paths.exports.ends_with("exports"));
    }
}
