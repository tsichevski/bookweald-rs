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

    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
    #[serde(default = "default_max_component_len")]
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

fn default_dry_run() -> bool {
    false
}

fn default_max_component_len() -> usize {
    0
}

fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

fn default_drop_existing_log_file_on_start() -> bool {
    false
}

fn default_library_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join("books/incoming"))
        .unwrap_or_else(|| PathBuf::from("~/books/incoming"))
}

fn default_target_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join("books/organized"))
        .unwrap_or_else(|| PathBuf::from("~/books/organized"))
}

impl Default for BookwealdConfig {
    fn default() -> Self {
        Self {
            library_dir: default_library_dir(),
            target_dir: default_target_dir(),
            dry_run: default_dry_run(),
            max_component_len: default_max_component_len(),
            jobs: default_jobs(),
            log_file: None,
            blacklist: None,
            alias_file: None,
            drop_existing_log_file_on_start: default_drop_existing_log_file_on_start(),
            log_level: None,
            database: DatabaseConfig::default(),
        }
    }
}

// -------------
// Loading logic
// -------------

impl BookwealdConfig {
    fn eff_location(location: Option<PathBuf>) -> anyhow::Result<PathBuf> {
        match location {
            Some(l) => Ok(l),
            None => {
                let local = Path::new("config.json");
                if local.exists() {
                    return Ok(PathBuf::from(local));
                }

                if let Some(mut p) = dirs::config_dir() {
                    p.push("bookweald/config.json");
                    if p.exists() {
                        return Ok(p);
                    }
                }

                anyhow::bail!(
                    "No config.json found.\nRun `bookweald init` (or `bookweald init --force`)"
                );
            }
        }
    }

    pub fn load(location: Option<PathBuf>) -> anyhow::Result<Self> {
        let path = Self::eff_location(location)?;
        Self::load_from(&path)
    }

    fn load_from(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e))?;

        let cfg: BookwealdConfig = json5::from_str(&content).map_err(|e| {
            let (line, column) = match e.position() {
                Some(pos) => (pos.line, pos.column),
                None => (0, 0),
            };

            anyhow::anyhow!(
                "❌ Invalid configuration in {}\n\
                 → Line {}, Column {}\n\
                 Error: {}\n\n\
                 Run `bookweald init --force` to regenerate a clean default.",
                path.display(),
                line,
                column,
                e
            )
        })?;

        Ok(cfg)
    }

    /// Create default config.json
    pub fn create_default(location: Option<PathBuf>, overwrite: bool) -> anyhow::Result<bool> {
        let path = Self::eff_location(location)?;
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
}
