use std::path::{Path, PathBuf};

pub const DEFAULT_REPO: &str = "https://raw.githubusercontent.com/pristochelovek097/wpm-repo/main";

#[derive(Clone, Debug)]
pub struct Config {
    /// Alternate root, useful for building images: `--root ./rootfs`.
    pub root: PathBuf,
    pub repo: String,
    pub verify: bool,
    /// COMBINE defaults to glibc and does not pull the musl loader.
    pub musl: bool,
    pub reinstall: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root: PathBuf::from("/"),
            repo: DEFAULT_REPO.to_string(),
            verify: true,
            musl: false,
            reinstall: false,
        }
    }
}

impl Config {
    /// Map an absolute distro path into the configured root.
    pub fn resolve(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();
        let relative = path.strip_prefix("/").unwrap_or(path);
        self.root.join(relative)
    }

    pub fn usr(&self) -> PathBuf {
        self.resolve("/usr")
    }

    pub fn usr_bin(&self) -> PathBuf {
        self.resolve("/usr/bin")
    }

    pub fn usr_lib(&self) -> PathBuf {
        self.resolve("/usr/lib")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.resolve("/var/cache/wpm")
    }

    /// Persistent state lives in /var/lib, not in the disposable download cache.
    pub fn state_dir(&self) -> PathBuf {
        self.resolve("/var/lib/wpm")
    }

    pub fn index_path(&self) -> PathBuf {
        self.cache_dir().join("Packages")
    }

    pub fn db_path(&self) -> PathBuf {
        self.state_dir().join("installed.list")
    }

    /// Old wpm kept the database in the cache directory; read it for upgrades.
    pub fn legacy_db_path(&self) -> PathBuf {
        self.cache_dir().join("installed.list")
    }

    pub fn manifests_dir(&self) -> PathBuf {
        self.state_dir().join("manifests")
    }

    pub fn url(&self, relative: &str) -> String {
        format!(
            "{}/{}",
            self.repo.trim_end_matches('/'),
            relative.trim_start_matches('/')
        )
    }
}
