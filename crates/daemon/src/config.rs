use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Preferences {
    pub start_runners_on_launch: bool,
    pub notify_status_changes: bool,
    pub notify_job_completions: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            start_runners_on_launch: false,
            notify_status_changes: true,
            notify_job_completions: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    base_dir: PathBuf,
    #[serde(default)]
    pub preferences: Preferences,
}

impl Default for Config {
    fn default() -> Self {
        let home = dirs::home_dir().expect("no home directory");
        Self {
            base_dir: home.join(".homerun"),
            preferences: Preferences::default(),
        }
    }
}

impl Config {
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            preferences: Preferences::default(),
        }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn socket_path(&self) -> PathBuf {
        self.base_dir.join("daemon.sock")
    }

    pub fn runners_dir(&self) -> PathBuf {
        self.base_dir.join("runners")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.base_dir.join("cache")
    }

    pub fn log_dir(&self) -> PathBuf {
        self.base_dir.join("logs")
    }

    pub fn config_path(&self) -> PathBuf {
        self.base_dir.join("config.toml")
    }

    pub fn runners_json_path(&self) -> PathBuf {
        self.base_dir.join("runners.json")
    }

    pub fn history_dir(&self) -> PathBuf {
        self.base_dir.join("history")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_dir)?;
        std::fs::create_dir_all(self.runners_dir())?;
        std::fs::create_dir_all(self.cache_dir())?;
        std::fs::create_dir_all(self.log_dir())?;
        std::fs::create_dir_all(self.history_dir())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(
            config.socket_path(),
            dirs::home_dir().unwrap().join(".homerun/daemon.sock")
        );
        assert_eq!(
            config.runners_dir(),
            dirs::home_dir().unwrap().join(".homerun/runners")
        );
        assert_eq!(
            config.cache_dir(),
            dirs::home_dir().unwrap().join(".homerun/cache")
        );
        assert_eq!(
            config.log_dir(),
            dirs::home_dir().unwrap().join(".homerun/logs")
        );
    }

    #[test]
    fn test_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = Config::with_base_dir(dir.path().join(".homerun"));
        config.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(config, loaded);
    }

    #[test]
    fn test_config_with_preferences_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut config = Config::with_base_dir(dir.path().join(".homerun"));
        config.preferences.notify_status_changes = false;
        config.preferences.start_runners_on_launch = true;
        config.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(config.preferences, loaded.preferences);
    }
}
