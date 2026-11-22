//! Small filesystem helpers shared by `wpm` and `wpm-repo`.

use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Recursively collect regular files below `dir`, sorted for reproducible
/// output. Symlinks are intentionally skipped.
pub fn walk_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    fn visit(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                visit(&path, out)?;
            } else if file_type.is_file() {
                out.push(path);
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    visit(dir, &mut files)?;
    files.sort();
    Ok(files)
}

pub fn is_elf(path: &Path) -> bool {
    let mut magic = [0u8; 4];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .map(|_| magic == *b"\x7fELF")
        .unwrap_or(false)
}

pub fn chmod(path: &Path, mode: u32) {
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

pub fn remove_path(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Move a file, falling back to copy+remove across filesystem boundaries.
pub fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(from, to)?;
            fs::remove_file(from)
        }
    }
}

/// Extensions that should never be marked executable.
pub fn has_non_executable_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "so" | "a" | "la" | "h" | "hpp" | "pc" | "txt" | "md" | "cmake"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn walks_and_sorts() {
        let dir = std::env::temp_dir().join(format!("wpm-core-walk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        File::create(dir.join("b")).unwrap();
        File::create(dir.join("sub").join("a")).unwrap();

        let files = walk_files(&dir).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[1].ends_with("sub/a"));

        fs::remove_dir_all(&dir).unwrap();
    }
}
