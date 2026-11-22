//! Parser for the Debian-ish `Packages` index used by wpm-repo.
//!
//! The on-disk format is a sequence of stanzas separated by blank lines:
//!
//! ```text
//! Package: bash
//! Version: 1.0
//! Type: bin
//! ...
//! ```

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub pkg_type: String,
    pub filename: String,
    pub size: u64,
    pub md5: String,
    pub destdir: String,
    /// "yes"/"no" in generated indexes; "so"/"a"/"both" are also accepted.
    pub libc: String,
    pub so_libc: String,
    pub a_libc: String,
    pub depends: Vec<String>,
}

impl Package {
    pub fn is_archive(&self) -> bool {
        let f = self.filename.to_ascii_lowercase();
        [
            ".tar.gz", ".tgz", ".tar.xz", ".txz", ".tar.bz2", ".tbz2", ".tar",
        ]
        .iter()
        .any(|suffix| f.ends_with(suffix))
    }

    pub fn is_library(&self) -> bool {
        matches!(self.pkg_type.as_str(), "lib" | "so" | "a" | "la")
    }

    pub fn is_executable(&self) -> bool {
        matches!(self.pkg_type.as_str(), "bin" | "script" | "dir")
    }

    /// Destination below `/usr`, mirroring wpm's historic layout.
    pub fn effective_destdir(&self) -> String {
        if !self.destdir.is_empty() {
            return self.destdir.clone();
        }
        if self.is_library() {
            "lib".to_string()
        } else {
            "bin".to_string()
        }
    }
}

pub fn parse_index(text: &str) -> Vec<Package> {
    let mut packages = Vec::new();
    let mut current = Package::default();
    let mut in_stanza = false;

    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');

        if line.trim().is_empty() {
            if in_stanza {
                packages.push(std::mem::take(&mut current));
                in_stanza = false;
            }
            continue;
        }

        // Continuation lines, comments, and repo markers are not fields.
        if line.starts_with([' ', '\t', '#']) {
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        in_stanza = true;

        match key {
            "Package" => current.name = value.to_string(),
            "Version" => current.version = value.to_string(),
            "Type" => current.pkg_type = value.to_string(),
            "Filename" => current.filename = value.to_string(),
            "Size" => current.size = value.parse().unwrap_or(0),
            "MD5sum" | "MD5" => current.md5 = value.to_string(),
            "DestDir" => current.destdir = value.to_string(),
            "libc" => current.libc = value.to_string(),
            "so libc" => current.so_libc = value.to_string(),
            "a libc" => current.a_libc = value.to_string(),
            "Depends" => {
                current.depends = value
                    .split(',')
                    .flat_map(|chunk| {
                        // Strip version constraints: "foo (>= 1.0)" -> "foo"
                        let head = chunk.split('(').next().unwrap_or("").trim();
                        head.split_whitespace()
                            .map(str::to_string)
                            .collect::<Vec<_>>()
                    })
                    .filter(|name| !name.is_empty())
                    .collect();
            }
            _ => {}
        }
    }

    if in_stanza {
        packages.push(current);
    }

    packages
}

pub fn find<'a>(packages: &'a [Package], name: &str) -> Option<&'a Package> {
    packages.iter().find(|pkg| pkg.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stanzas_and_depends() {
        let text = "\
###AUTO_START###
Package: bash
Version: 1.0
Type: bin
Filename: pool/main/bash
Size: 42
MD5sum: aa
DestDir: bin
libc: no
Depends: libc, ncurses (>= 6)

Package: libc
Version: 2.0
Type: so
";
        let pkgs = parse_index(text);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "bash");
        assert_eq!(pkgs[0].size, 42);
        assert_eq!(pkgs[0].depends, vec!["libc", "ncurses"]);
        assert!(pkgs[1].is_library());
    }

    #[test]
    fn detects_archives() {
        let mut pkg = Package::default();
        pkg.filename = "pool/main/xorg.tar.gz".into();
        assert!(pkg.is_archive());
        pkg.filename = "pool/main/bash".into();
        assert!(!pkg.is_archive());
    }
}
