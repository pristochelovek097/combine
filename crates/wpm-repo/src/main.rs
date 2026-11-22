//! wpm-repo: regenerate the auto section of a wpm package index.
//!
//! Replaces the old bash + Perl GitHub Action. The manual header and footer
//! around `###AUTO_START###` / `###AUTO_END###` are preserved untouched.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use wpm_core::packages;
use wpm_core::Result;
use wpm_core::{fsutil, md5};

const START: &str = "###AUTO_START###";
const END: &str = "###AUTO_END###";

#[derive(Debug)]
struct Options {
    root: PathBuf,
}

impl Options {
    fn pool(&self) -> PathBuf {
        self.root.join("pool/main")
    }

    fn packages_file(&self) -> PathBuf {
        self.root.join("Packages")
    }
}

fn main() {
    process::exit(run());
}

fn run() -> i32 {
    let args: Vec<String> = env::args().skip(1).collect();
    let (command, options) = match parse(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("wpm-repo: {message}");
            return 1;
        }
    };

    let result = match command.as_str() {
        "gen" | "generate" | "update" => generate(&options),
        "check" | "verify" => check(&options),
        "help" | "--help" | "-h" => {
            usage();
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    };

    match result {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("wpm-repo: {message}");
            1
        }
    }
}

fn parse(args: Vec<String>) -> Result<(String, Options)> {
    let mut root = PathBuf::from(".");
    let mut command: Option<String> = None;
    let mut args = args.into_iter();

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => {
                root = PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--root requires a directory".to_string())?,
                );
            }
            "-h" | "--help" => command = Some("help".to_string()),
            other if other.starts_with('-') => {
                return Err(format!("unknown option: {other}"));
            }
            other => {
                if command.is_some() {
                    return Err(format!("unexpected argument: {other}"));
                }
                command = Some(other.to_string());
            }
        }
    }

    Ok((
        command.unwrap_or_else(|| "help".to_string()),
        Options { root },
    ))
}

fn generate(options: &Options) -> Result<()> {
    let pool = options.pool();
    if !pool.is_dir() {
        return Err(format!("{} is not a directory", pool.display()));
    }

    let files = fsutil::walk_files(&pool)
        .map_err(|error| format!("cannot walk {}: {error}", pool.display()))?;

    let mut block = String::new();
    block.push_str(START);
    block.push('\n');

    let mut count = 0usize;
    for file in &files {
        let filename = relative(&options.root, file);
        let inner = relative(&pool, file);

        // GCC's build leftovers are not packages.
        if filename.contains("pool/main/lib/a/gcc/") {
            continue;
        }

        block.push_str(&describe(&filename, &inner, file)?);
        count += 1;
    }

    block.push_str(END);

    let packages_file = options.packages_file();
    let existing = fs::read_to_string(&packages_file).unwrap_or_default();
    let updated = splice(&existing, &block);
    fs::write(&packages_file, updated)
        .map_err(|error| format!("cannot write {}: {error}", packages_file.display()))?;

    println!(
        "wpm-repo: wrote {count} package(s) to {}",
        packages_file.display()
    );
    Ok(())
}

fn describe(filename: &str, inner: &str, file: &Path) -> Result<String> {
    let metadata =
        fs::metadata(file).map_err(|error| format!("cannot stat {}: {error}", file.display()))?;
    let digest =
        md5::file_hex(file).map_err(|error| format!("cannot hash {}: {error}", file.display()))?;

    let base = Path::new(filename)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    let (name, pkg_type, destdir, libc) = classify(inner, &base);

    Ok(format!(
        "Package: {name}\nVersion: 1.0\nType: {pkg_type}\nFilename: {filename}\nSize: {}\nMD5sum: {digest}\nDestDir: {destdir}\nlibc: {libc}\n\n",
        metadata.len()
    ))
}

/// Return (package name, type, DestDir, libc) using the same rules as the
/// original shell generator, with tar.xz/tgz support added.
fn classify(inner: &str, base: &str) -> (String, String, String, String) {
    if inner.starts_with("lib/so/") {
        return (base.into(), "so".into(), "lib".into(), "yes".into());
    }
    if inner.starts_with("lib/a/") {
        return (base.into(), "a".into(), "lib".into(), "yes".into());
    }
    if base == "ld-musl-x86_64.so.1" {
        return (base.into(), "so".into(), "lib".into(), "yes".into());
    }

    for suffix in [
        ".tar.gz", ".tgz", ".tar.xz", ".txz", ".tar.bz2", ".tbz2", ".tar",
    ] {
        if let Some(stem) = base.strip_suffix(suffix) {
            let destdir = if stem == "gcc" { "/" } else { "bin" };
            return (stem.into(), "dir".into(), destdir.into(), "no".into());
        }
    }

    let extension = Path::new(base)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let (pkg_type, destdir) = match extension.as_str() {
        "a" => ("lib", "lib"),
        "so" => ("so", "lib"),
        "la" => ("la", "lib"),
        "h" | "hpp" => ("dev", "include"),
        _ => ("bin", "bin"),
    };

    let libc = if base.starts_with("ld-musl-") {
        "yes"
    } else {
        "no"
    };

    (base.into(), pkg_type.into(), destdir.into(), libc.into())
}

fn relative(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Replace the marked block, preserving any manual header/footer.
fn splice(existing: &str, block: &str) -> String {
    if let Some(start) = existing.find(START) {
        if let Some(offset) = existing[start..].find(END) {
            let end = start + offset + END.len();
            return format!("{}{}{}", &existing[..start], block, &existing[end..]);
        }
    }

    if existing.trim().is_empty() {
        format!("{block}\n")
    } else {
        let separator = if existing.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };
        format!("{}{separator}{block}\n", existing.trim_end())
    }
}

fn check(options: &Options) -> Result<()> {
    let packages_file = options.packages_file();
    let text = fs::read_to_string(&packages_file)
        .map_err(|error| format!("cannot read {}: {error}", packages_file.display()))?;
    let index = packages::parse_index(&text);

    let mut failures = 0usize;
    for package in &index {
        if package.filename.is_empty() {
            continue;
        }
        let path = options.root.join(&package.filename);

        let problem = match fs::metadata(&path) {
            Err(error) => Some(format!("cannot stat {}: {error}", path.display())),
            Ok(metadata) => {
                if package.size > 0 && metadata.len() != package.size {
                    Some(format!(
                        "size mismatch: index {}, disk {}",
                        package.size,
                        metadata.len()
                    ))
                } else if !package.md5.is_empty() {
                    match md5::file_hex(&path) {
                        Ok(digest) if digest.eq_ignore_ascii_case(&package.md5) => None,
                        Ok(digest) => Some(format!(
                            "md5 mismatch: index {}, disk {digest}",
                            package.md5
                        )),
                        Err(error) => Some(format!("cannot hash {}: {error}", path.display())),
                    }
                } else {
                    None
                }
            }
        };

        match problem {
            None => println!("ok    {}", package.name),
            Some(problem) => {
                failures += 1;
                println!("FAIL  {}: {problem}", package.name);
            }
        }
    }

    if failures > 0 {
        Err(format!(
            "{failures} of {} package(s) failed validation",
            index.len()
        ))
    } else {
        println!("All {} package(s) OK", index.len());
        Ok(())
    }
}

fn usage() {
    println!(
        "\
wpm-repo {} - COMBINE repository tool

USAGE:
    wpm-repo gen [--root DIR]     Regenerate the ###AUTO### block in Packages
    wpm-repo check [--root DIR]   Verify size/MD5 of every indexed file

The generator scans pool/main, skips lib/a/gcc build leftovers, and preserves
any manual header and footer around the markers.
",
        env!("CARGO_PKG_VERSION")
    );
}
