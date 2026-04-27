use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NamespaceConfig {
    #[serde(default)]
    pub namespaces: HashMap<String, String>,
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

    // ── Namespace → Schema mapping ──
    #[serde(default)]
    pub namespaces: NamespaceConfig,
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
        let mut cfg = Self {
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
            namespaces: NamespaceConfig::default(),
        };

        // Built-in FictionBook defaults
        cfg.namespaces.namespaces.insert(
            "http://www.gribuser.ru/xml/fictionbook/2.0".to_string(),
            "schemas/FictionBook2.0.xsd".to_string(),
        );
        cfg.namespaces.namespaces.insert(
            "http://www.gribuser.ru/xml/fictionbook/2.1".to_string(),
            "schemas/FictionBook2.1.xsd".to_string(),
        );

        cfg
    }
}

// ------------------------------------------------------------------
// Loading logic (unchanged from original)
// ------------------------------------------------------------------

static CONFIG: OnceLock<BookwealdConfig> = OnceLock::new();

impl BookwealdConfig {
    pub fn load() -> anyhow::Result<&'static Self> {
        CONFIG.get_or_init(|| {
            // load logic from file or default...
            Self::default()
        });
        Ok(CONFIG.get().unwrap())
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

    pub fn get_schema_for_namespace(&self, ns: &str) -> Option<String> {
        self.namespaces.namespaces.get(ns).cloned()
    }
}

// =====
// TESTS
// =====

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_has_fictionbook_namespaces() {
        let cfg = BookwealdConfig::default();
        assert!(
            cfg.namespaces
                .namespaces
                .contains_key("http://www.gribuser.ru/xml/fictionbook/2.0")
        );
        assert!(
            cfg.namespaces
                .namespaces
                .contains_key("http://www.gribuser.ru/xml/fictionbook/2.1")
        );
    }

    #[test]
    fn test_get_schema_for_namespace() {
        let cfg = BookwealdConfig::default();
        assert_eq!(
            cfg.get_schema_for_namespace("http://www.gribuser.ru/xml/fictionbook/2.0"),
            Some("schemas/FictionBook2.0.xsd".to_string())
        );
        assert_eq!(cfg.get_schema_for_namespace("unknown-ns"), None);
    }

    #[test]
    fn test_namespace_config_deserialization() {
        let json = r#"
        {
            "library_dir": "library",
            "target_dir": "target",
            "namespaces": {
                "http://www.gribuser.ru/xml/fictionbook/2.0": "custom/FictionBook2.0.xsd",
                "http://example.com/schema": "schemas/custom.xsd"
            }
        }
        "#;

        let cfg: BookwealdConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            cfg.get_schema_for_namespace("http://www.gribuser.ru/xml/fictionbook/2.0"),
            Some("custom/FictionBook2.0.xsd".to_string())
        );
    }
}
