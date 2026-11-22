use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use wpm_core::fsutil;
use wpm_core::md5;
use wpm_core::packages::{self, Package};
use wpm_core::Result;

use crate::config::Config;
use crate::db;
use crate::download;

pub fn update(config: &Config) -> Result<Vec<Package>> {
    let url = config.url("Packages");
    println!("Updating package index from {}", config.repo);
    download::fetch(&url, &config.index_path(), None)?;
    let index = load_index(config)?;
    println!("Index updated: {} package(s)", index.len());
    Ok(index)
}

pub fn ensure_index(config: &Config) -> Result<Vec<Package>> {
    let path = config.index_path();
    if path.is_file() {
        if let Ok(index) = load_index(config) {
            if !index.is_empty() {
                return Ok(index);
            }
        }
    }
    update(config)
}

fn load_index(config: &Config) -> Result<Vec<Package>> {
    let path = config.index_path();
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e} (try `wpm update`)", path.display()))?;
    Ok(packages::parse_index(&text))
}

fn read_db(config: &Config) -> Vec<db::Entry> {
    let entries = db::read(&config.db_path());
    if entries.is_empty() {
        // Transparency for systems still carrying the old cache-based DB.
        db::read(&config.legacy_db_path())
    } else {
        entries
    }
}

fn db_contains(config: &Config, name: &str) -> bool {
    read_db(config).iter().any(|entry| entry.name == name)
}

pub fn list_packages(config: &Config) -> Result<()> {
    ensure_index(config)?;
    let path = config.index_path();
    let text =
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    print!("{text}");
    Ok(())
}

pub fn list_installed(config: &Config) -> Result<()> {
    let entries = read_db(config);
    if entries.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }
    println!(
        "{:<22} {:<12} {:<8} {}",
        "PACKAGE", "VERSION", "TYPE", "DEST"
    );
    for entry in entries {
        println!(
            "{:<22} {:<12} {:<8} {}",
            entry.name, entry.version, entry.pkg_type, entry.dest
        );
    }
    Ok(())
}

pub fn search(config: &Config, query: &str) -> Result<()> {
    let index = ensure_index(config)?;
    let query = query.to_ascii_lowercase();
    let matches: Vec<&Package> = index
        .iter()
        .filter(|pkg| pkg.name.to_ascii_lowercase().contains(&query))
        .collect();

    println!("\nSearch results for '{query}':\n");
    if matches.is_empty() {
        println!("  No packages found");
        return Ok(());
    }
    for pkg in &matches {
        println!(
            "  {:<22} {:<10} [{:<4}] {:>10}",
            pkg.name,
            pkg.version,
            pkg.pkg_type,
            download::human(pkg.size)
        );
    }
    println!("\nFound {} package(s)", matches.len());
    Ok(())
}

pub fn info(config: &Config, name: &str) -> Result<()> {
    let index = ensure_index(config)?;
    let pkg = packages::find(&index, name)
        .ok_or_else(|| format!("package '{name}' was not found in the index"))?;
    let installed = read_db(config)
        .into_iter()
        .find(|entry| entry.name == pkg.name);
    let libc = if pkg.libc.is_empty() { "no" } else { &pkg.libc };

    println!("Package:     {}", pkg.name);
    println!("Version:     {}", pkg.version);
    println!("Type:        {}", pkg.pkg_type);
    println!("Filename:    {}", pkg.filename);
    println!("Size:        {}", download::human(pkg.size));
    println!("MD5sum:      {}", pkg.md5);
    println!("DestDir:     /usr/{}", pkg.effective_destdir());
    println!("libc:        {libc}");
    if !pkg.depends.is_empty() {
        println!("Depends:     {}", pkg.depends.join(", "));
    }
    match installed {
        Some(entry) => println!("Status:      installed ({})", entry.version),
        None => println!("Status:      not installed"),
    }
    Ok(())
}

pub fn add(config: &Config, names: &[String]) -> Result<()> {
    let index = ensure_index(config)?;
    let mut done = Vec::new();
    for name in names {
        install_recursive(config, &index, name, &mut done, &mut Vec::new())?;
    }
    Ok(())
}

fn install_recursive(
    config: &Config,
    index: &[Package],
    name: &str,
    done: &mut Vec<String>,
    stack: &mut Vec<String>,
) -> Result<()> {
    if done.iter().any(|item| item == name) {
        return Ok(());
    }
    if stack.iter().any(|item| item == name) {
        eprintln!("wpm: warning: dependency cycle at '{name}', skipping");
        return Ok(());
    }

    let pkg = packages::find(index, name).ok_or_else(|| missing_package(index, name))?;

    for dependency in &pkg.depends {
        if packages::find(index, dependency).is_some() {
            install_recursive(config, index, dependency, done, stack)?;
        } else if !db_contains(config, dependency) {
            eprintln!(
                "wpm: warning: '{}' depends on '{}', which is not in the index",
                pkg.name, dependency
            );
        }
    }

    stack.push(name.to_string());
    install_one(config, pkg)?;
    stack.pop();
    done.push(name.to_string());
    Ok(())
}

fn manifest_path(config: &Config, name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+') {
                c
            } else {
                '_'
            }
        })
        .collect();
    config.manifests_dir().join(format!("{safe}.list"))
}

fn write_manifest(config: &Config, name: &str, files: &[PathBuf]) -> Result<()> {
    let directory = config.manifests_dir();
    fs::create_dir_all(&directory)
        .map_err(|e| format!("cannot create {}: {e}", directory.display()))?;
    let mut text = String::new();
    for file in files {
        let stored = file
            .strip_prefix(&config.root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        text.push_str(&stored);
        text.push('\n');
    }
    let path = manifest_path(config, name);
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, text)
        .map_err(|e| format!("cannot write {}: {e}", temporary.display()))?;
    fs::rename(&temporary, &path).map_err(|e| format!("cannot replace {}: {e}", path.display()))?;
    Ok(())
}

fn read_manifest(config: &Config, name: &str) -> Vec<PathBuf> {
    fs::read_to_string(manifest_path(config, name))
        .map(|text| {
            text.lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| config.root.join(line.trim()))
                .collect()
        })
        .unwrap_or_default()
}

/// Remove directories that became empty, stopping at the first non-empty one.
fn prune_empty_parents(config: &Config, path: &Path) {
    let mut current = path.parent();
    while let Some(directory) = current {
        if directory == config.root || !directory.starts_with(config.usr()) {
            break;
        }
        if fs::remove_dir(directory).is_err() {
            break;
        }
        current = directory.parent();
    }
}

fn missing_package(index: &[Package], name: &str) -> String {
    let mut message = format!("package '{name}' was not found.\n\nAvailable packages:\n");
    for pkg in index {
        message.push_str(&format!(
            "  - {:<22} {:<10} {}\n",
            pkg.name, pkg.version, pkg.pkg_type
        ));
    }
    message
}

fn install_one(config: &Config, pkg: &Package) -> Result<()> {
    let database = config.db_path();
    if !config.reinstall && db_contains(config, &pkg.name) {
        println!(
            "{} {} is already installed (use --reinstall)",
            pkg.name, pkg.version
        );
        return Ok(());
    }

    let pkg_type = if pkg.pkg_type.is_empty() {
        "bin"
    } else {
        &pkg.pkg_type
    };
    println!("\n{} ({})", pkg.name, pkg_type);

    install_libc_dependencies(config, pkg)?;

    let url = package_url(config, pkg);
    let cache = config.cache_dir();
    fs::create_dir_all(&cache).map_err(|e| format!("cannot create {}: {e}", cache.display()))?;
    let download_path = cache.join(format!("{}.download", file_name(&pkg.name)));
    download::fetch(
        &url,
        &download_path,
        if pkg.size > 0 { Some(pkg.size) } else { None },
    )?;

    if config.verify {
        if pkg.md5.is_empty() {
            eprintln!(
                "wpm: warning: {} has no MD5sum, skipping verification",
                pkg.name
            );
        } else {
            let actual = md5::file_hex(&download_path)
                .map_err(|e| format!("cannot hash {}: {e}", download_path.display()))?;
            if !actual.eq_ignore_ascii_case(&pkg.md5) {
                let _ = fs::remove_file(&download_path);
                return Err(format!(
                    "checksum mismatch for {}: expected {}, got {}",
                    pkg.name, pkg.md5, actual
                ));
            }
            println!("Checksum OK ({})", &actual[..8]);
        }
    }

    let destination = config
        .usr()
        .join(pkg.effective_destdir().trim_start_matches('/'));
    fs::create_dir_all(&destination)
        .map_err(|e| format!("cannot create {}: {e}", destination.display()))?;

    let mut installed_files = Vec::new();
    if pkg.is_archive() {
        let before: BTreeSet<PathBuf> = fsutil::walk_files(&destination)
            .unwrap_or_default()
            .into_iter()
            .collect();
        println!("Extracting archive into {}", destination.display());
        extract_archive(&download_path, &destination)?;
        let _ = fs::remove_file(&download_path);
        chmod_tree(&destination);
        installed_files = fsutil::walk_files(&destination)
            .unwrap_or_default()
            .into_iter()
            .filter(|path| !before.contains(path))
            .collect();
    } else {
        let target = destination.join(file_name(&pkg.name));
        fsutil::move_file(&download_path, &target)
            .map_err(|e| format!("cannot install {}: {e}", target.display()))?;
        let mode = if pkg.is_executable() { 0o755 } else { 0o644 };
        fsutil::chmod(&target, mode);
        installed_files.push(target);
    }

    db::record(&database, pkg)?;
    write_manifest(config, &pkg.name, &installed_files)?;
    println!("Installed {} -> {}", pkg.name, destination.display());

    if pkg.is_executable() {
        check_dir_ldd(&destination);
    }
    Ok(())
}

fn package_url(config: &Config, pkg: &Package) -> String {
    if !pkg.filename.is_empty() {
        return config.url(&pkg.filename);
    }
    let pool = if pkg.is_library() {
        "pool/main/libs"
    } else {
        "pool/main"
    };
    config.url(&format!("{pool}/{}", pkg.name))
}

fn install_libc_dependencies(config: &Config, pkg: &Package) -> Result<()> {
    // "yes"/"no" come from the generated index and mean "is a libc package",
    // not "requires libc". Only so/a/both request extra runtime libraries.
    if pkg.libc.is_empty() || matches!(pkg.libc.as_str(), "no" | "yes") {
        return Ok(());
    }
    let need_so = matches!(pkg.libc.as_str(), "so" | "both");
    let need_a = matches!(pkg.libc.as_str(), "a" | "both");

    if need_so && !pkg.so_libc.is_empty() {
        install_runtime_library(config, &pkg.so_libc)?;
    }
    if need_a && !pkg.a_libc.is_empty() {
        install_runtime_library(config, &pkg.a_libc)?;
    }
    Ok(())
}

fn install_runtime_library(config: &Config, repo_path: &str) -> Result<()> {
    let name = file_name(repo_path);
    if name.is_empty() {
        return Ok(());
    }
    let destination = config.usr_lib().join(&name);
    if destination.is_file() {
        return Ok(());
    }
    if name.starts_with("ld-musl") && !config.musl {
        println!("Skipping musl loader {name} (glibc profile; pass --musl to allow)");
        return Ok(());
    }

    println!("Installing required library: {name}");
    let cache = config.cache_dir();
    fs::create_dir_all(&cache).map_err(|e| format!("cannot create {}: {e}", cache.display()))?;
    let temporary = cache.join(format!("{name}.download"));
    download::fetch(&config.url(repo_path), &temporary, None)?;

    fs::create_dir_all(config.usr_lib())
        .map_err(|e| format!("cannot create {}: {e}", config.usr_lib().display()))?;
    fsutil::move_file(&temporary, &destination)
        .map_err(|e| format!("cannot install {}: {e}", destination.display()))?;
    fsutil::chmod(&destination, 0o644);
    Ok(())
}

fn extract_archive(archive: &Path, destination: &Path) -> Result<()> {
    let status = Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(destination)
        .status();
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("tar failed with {status}")),
        Err(error) => Err(format!("cannot run tar: {error}")),
    }
}

fn chmod_tree(directory: &Path) {
    for file in fsutil::walk_files(directory).unwrap_or_default() {
        if !fsutil::has_non_executable_extension(&file) {
            fsutil::chmod(&file, 0o755);
        }
    }
}

fn check_dir_ldd(directory: &Path) {
    for file in fsutil::walk_files(directory).unwrap_or_default() {
        if !fsutil::is_elf(&file) {
            continue;
        }
        let output = Command::new("ldd").arg(&file).output();
        let Ok(output) = output else { continue };
        let report = String::from_utf8_lossy(&output.stdout);
        if report.contains("not found") {
            eprintln!(
                "wpm: warning: missing shared libraries for {}:\n{}",
                file.display(),
                report.trim()
            );
        }
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

fn target_path(config: &Config, entry: &db::Entry) -> PathBuf {
    if !entry.dest.is_empty() {
        config
            .usr()
            .join(entry.dest.trim_matches('/'))
            .join(&entry.name)
    } else if matches!(entry.pkg_type.as_str(), "lib" | "so" | "a" | "la") {
        config.usr_lib().join(&entry.name)
    } else {
        config.usr_bin().join(&entry.name)
    }
}

pub fn remove(config: &Config, names: &[String]) -> Result<()> {
    let database = config.db_path();
    let mut entries = read_db(config);

    for name in names {
        let Some(position) = entries.iter().position(|entry| &entry.name == name) else {
            return Err(format!("package '{name}' is not installed"));
        };
        let entry = entries[position].clone();
        let mut targets = read_manifest(config, &entry.name);
        if targets.is_empty() {
            targets.push(target_path(config, &entry));
        }

        println!("Removing {} ({})...", entry.name, entry.version);
        let mut removed = 0usize;
        for target in &targets {
            if target.exists() {
                fsutil::remove_path(target)
                    .map_err(|e| format!("cannot remove {}: {e}", target.display()))?;
                removed += 1;
            }
        }
        for target in &targets {
            prune_empty_parents(config, target);
        }
        if removed == 0 {
            println!("Warning: no files found on disk, removing from the database only.");
        }
        let _ = fs::remove_file(manifest_path(config, &entry.name));

        entries.remove(position);
        db::write(&database, &entries)?;
        println!("Removed {} ({removed} file(s))", name);
    }
    Ok(())
}

pub fn verify(config: &Config) -> Result<()> {
    let entries = read_db(config);
    if entries.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    let mut missing = 0;
    for entry in &entries {
        let mut targets = read_manifest(config, &entry.name);
        if targets.is_empty() {
            targets.push(target_path(config, entry));
        }

        let absent: Vec<&PathBuf> = targets.iter().filter(|path| !path.exists()).collect();
        if absent.is_empty() {
            println!("ok    {:<22} {}", entry.name, entry.version);
        } else {
            missing += 1;
            for path in absent {
                println!("MISS  {:<22} {}", entry.name, path.display());
            }
        }
    }

    if missing > 0 {
        Err(format!(
            "{missing} of {} installed package(s) have missing files",
            entries.len()
        ))
    } else {
        println!("\nAll {} installed package(s) verified.", entries.len());
        Ok(())
    }
}

pub fn clean(config: &Config) -> Result<()> {
    let cache = config.cache_dir();
    println!("Delete cache");
    if !cache.exists() {
        println!("Success! (cache empty)");
        return Ok(());
    }
    for file in fsutil::walk_files(&cache).unwrap_or_default() {
        let _ = fs::remove_file(file);
    }
    fs::remove_dir_all(&cache).map_err(|e| format!("cannot remove {}: {e}", cache.display()))?;
    println!("Success!");
    Ok(())
}
