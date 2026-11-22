use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use wpm_core::Result;

/// Download `url` into `output` with a progress display.
///
/// HTTPS transport is delegated to curl (preferred) or wget, which every
/// COMBINE image ships. Everything else — integrity, placement, rollback —
/// is handled in Rust.
pub fn fetch(url: &str, output: &Path, expected: Option<u64>) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }

    let temporary = PathBuf::from(format!("{}.part", output.display()));
    let _ = fs::remove_file(&temporary);

    let mut child = spawn_downloader(&temporary, url)?;
    let mut spin = 0usize;

    let status = loop {
        match child
            .try_wait()
            .map_err(|e| format!("cannot wait for downloader: {e}"))?
        {
            Some(status) => break status,
            None => {
                let current = fs::metadata(&temporary).map(|m| m.len()).unwrap_or(0);
                match expected.filter(|total| *total > 0) {
                    Some(total) => {
                        let percent = (current.saturating_mul(100) / total).min(100);
                        let filled = (percent / 5) as usize;
                        let bar = format!("{}{}", "=".repeat(filled), " ".repeat(20 - filled));
                        print!(
                            "\r[{bar}] {percent:3}%  {} / {}",
                            human(current),
                            human(total)
                        );
                    }
                    None => {
                        let frame = ['|', '/', '-', '\\'][spin % 4];
                        print!("\rDownloading {frame}  {}", human(current));
                    }
                }
                let _ = std::io::stdout().flush();
                spin += 1;
                thread::sleep(Duration::from_millis(120));
            }
        }
    };

    println!();

    if !status.success() {
        let _ = fs::remove_file(&temporary);
        return Err(format!("download failed: {url}"));
    }

    let downloaded = fs::metadata(&temporary).map(|m| m.len()).unwrap_or(0);
    if let Some(total) = expected.filter(|total| *total > 0) {
        if downloaded != total {
            eprintln!(
                "wpm: warning: size mismatch for {url}: index says {total}, got {downloaded}"
            );
        }
    }

    fs::rename(&temporary, output)
        .map_err(|e| format!("cannot move download into {}: {e}", output.display()))?;
    Ok(())
}

fn spawn_downloader(output: &Path, url: &str) -> Result<Child> {
    match Command::new("curl")
        .args([
            "-fsSL",
            "--retry",
            "2",
            "--connect-timeout",
            "20",
            "--max-time",
            "3600",
            "-o",
        ])
        .arg(output)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => Ok(child),
        Err(error) if error.kind() == ErrorKind::NotFound => match Command::new("wget")
            .args(["--no-check-certificate", "-q", "-O"])
            .arg(output)
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => Ok(child),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                Err("neither curl nor wget is installed".to_string())
            }
            Err(error) => Err(format!("cannot run wget: {error}")),
        },
        Err(error) => Err(format!("cannot run curl: {error}")),
    }
}

pub fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
