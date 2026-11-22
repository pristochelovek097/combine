use std::fs;
use std::path::Path;

use wpm_core::packages::Package;
use wpm_core::Result;

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub version: String,
    pub pkg_type: String,
    pub dest: String,
}

fn parse_line(line: &str) -> Option<Entry> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() {
        return None;
    }
    let mut parts = line.split('|');
    let name = parts.next()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(Entry {
        name: name.to_string(),
        version: parts.next().unwrap_or("-").trim().to_string(),
        pkg_type: parts.next().unwrap_or("bin").trim().to_string(),
        dest: parts.next().unwrap_or("bin").trim().to_string(),
    })
}

pub fn read(path: &Path) -> Vec<Entry> {
    fs::read_to_string(path)
        .map(|text| text.lines().filter_map(parse_line).collect())
        .unwrap_or_default()
}

pub fn write(path: &Path, entries: &[Entry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }

    let mut text = String::new();
    for entry in entries {
        text.push_str(&format!(
            "{}|{}|{}|{}\n",
            entry.name, entry.version, entry.pkg_type, entry.dest
        ));
    }

    let temporary = path.with_extension("tmp");
    fs::write(&temporary, text)
        .map_err(|e| format!("cannot write {}: {e}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))?;
    Ok(())
}

pub fn record(path: &Path, package: &Package) -> Result<()> {
    let mut entries = read(path);
    entries.retain(|entry| entry.name != package.name);
    entries.push(Entry {
        name: package.name.clone(),
        version: if package.version.is_empty() {
            "1.0".to_string()
        } else {
            package.version.clone()
        },
        pkg_type: if package.pkg_type.is_empty() {
            "bin".to_string()
        } else {
            package.pkg_type.clone()
        },
        dest: package.effective_destdir(),
    });
    write(path, &entries)
}
