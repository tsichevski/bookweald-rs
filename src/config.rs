use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ------------------------------------------------------------------
// DatabaseConfig
// ------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DatabaseConfig {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_user")]
    pub user: String,

    #[serde(default = "default_passwd")]
    pub passwd: String,

    #[serde(default = "default_name")]
    pub name: String,

    #[serde(default = "default_admin")]
    pub admin: String,

    #[serde(default = "default_admin_passwd")]
    pub admin_passwd: String,
}

fn default_host() -> String {
    "localhost".into()
}
fn default_port() -> u16 {
    5432
}
fn default_user() -> String {
    "books".into()
}
fn default_passwd() -> String {
    "books".into()
}
fn default_name() -> String {
    "books".into()
}
fn default_admin() -> String {
    "admin".into()
}
fn default_admin_passwd() -> String {
    "admin".into()
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            user: default_user(),
            passwd: default_passwd(),
            name: default_name(),
            admin: default_admin(),
            admin_passwd: default_admin_passwd(),
        }
    }
}

// ------------------------------------------------------------------
// BookwealdConfig
// ------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BookwealdConfig {
    pub library_dir: PathBuf,
    pub target_dir: PathBuf,

    pub dry_run: bool,
    pub max_component_len: usize,

    #[serde(default = "default_jobs")]
    pub jobs: usize,

    pub log_file: Option<PathBuf>,
    pub blacklist: Option<PathBuf>,
    pub alias_file: Option<PathBuf>,

    #[serde(default = "default_drop_existing_log_file_on_start")]
    pub drop_existing_log_file_on_start: bool,

    pub log_level: Option<String>,

    #[serde(default)]
    pub database: DatabaseConfig,
}

// --------------------- Default helpers ---------------------

fn default_jobs() -> usize {
    1
}

fn default_drop_existing_log_file_on_start() -> bool {
    false
}

pub fn default_library_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join("books/incoming"))
        .unwrap_or_else(|| PathBuf::from("~/books/incoming"))
}

pub fn default_target_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join("books/organized"))
        .unwrap_or_else(|| PathBuf::from("~/books/organized"))
}

impl Default for BookwealdConfig {
    fn default() -> Self {
        Self {
            library_dir: default_library_dir(),
            target_dir: default_target_dir(),
            dry_run: false,
            max_component_len: 0,
            jobs: default_jobs(),
            log_file: None,
            blacklist: None,
            drop_existing_log_file_on_start: default_drop_existing_log_file_on_start(),
            log_level: None,
            alias_file: None,
            database: DatabaseConfig::default(),
        }
    }
}

// --------------------- Public API ---------------------

impl BookwealdConfig {
    /// Load config (strict after init)
    pub fn load() -> anyhow::Result<Self> {
        if Path::new("config.json").exists() {
            return Self::load_from("config.json");
        }

        if let Some(mut p) = dirs::config_dir() {
            p.push("bookweald/config.json");
            if p.exists() {
                return Self::load_from(&p);
            }
        }

        anyhow::bail!("No config.json found.\nRun `bookweald init` (or `bookweald init --force`)");
    }

    fn load_from<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(&path)?;
        let mut cfg: BookwealdConfig = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Invalid config in {}: {}", path.as_ref().display(), e))?;

        cfg.resolve_paths();
        Ok(cfg)
    }

    /// Create default config.json
    pub fn create_default(overwrite: bool) -> anyhow::Result<bool> {
        let path = dirs::config_dir()
            .map(|mut p| {
                p.push("bookweald/config.json");
                p
            })
            .unwrap_or_else(|| PathBuf::from("config.json"));

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if path.exists() && !overwrite {
            println!(
                "Config already exists and --force was not used: {}",
                path.display()
            );
            return Ok(false);
        }

        let cfg = Self::default();
        let pretty = serde_json::to_string_pretty(&cfg)?;
        std::fs::write(&path, pretty + "\n")?;

        println!("✅ Created default configuration: {}", path.display());
        Ok(true)
    }

    /// Expand ~ and make paths absolute
    pub fn resolve_paths(&mut self) {
        if let Some(home) = dirs::home_dir() {
            fn expand(p: &mut PathBuf, home: &std::path::Path) {
                if let Some(s) = p.to_str() {
                    if let Some(rest) = s.strip_prefix('~') {
                        let rest = rest.trim_start_matches('/');
                        *p = home.join(rest);
                    }
                }
                if !p.is_absolute() {
                    if let Ok(cwd) = std::env::current_dir() {
                        *p = cwd.join(&*p);
                    }
                }
            }

            expand(&mut self.library_dir, &home);
            expand(&mut self.target_dir, &home);

            if let Some(ref mut f) = self.log_file {
                expand(f, &home);
            }
            if let Some(ref mut f) = self.blacklist {
                expand(f, &home);
            }
            if let Some(ref mut f) = self.alias_file {
                expand(f, &home);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = BookwealdConfig::default();
        // println!("Default: {:?}", &cfg);
        assert!(
            cfg.library_dir.ends_with("books/incoming")
                || cfg.library_dir.to_string_lossy().contains("incoming")
        );
        assert!(
            cfg.target_dir.ends_with("books/organized")
                || cfg.target_dir.to_string_lossy().contains("organized")
        );
        assert_eq!(cfg.jobs, 1);
        assert_eq!(cfg.database.host, "localhost");
        assert_eq!(cfg.database.port, 5432);
    }

    #[test]
    fn test_create_default_and_load() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;

        // Simulate XDG for test
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        BookwealdConfig::create_default(true)?;

        let mut cfg = BookwealdConfig::load()?;
        cfg.resolve_paths();

        assert!(cfg.library_dir.is_absolute());
        assert!(cfg.target_dir.is_absolute());

        Ok(())
    }
}
