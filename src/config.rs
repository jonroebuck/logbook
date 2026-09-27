use std::{env, path::PathBuf};

use thiserror::Error;

const DEFAULT_DB_PATH: &str = "/data/logbook.db";
const DEFAULT_PORT: u16 = 8080;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub db_path: PathBuf,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let db_path = env::var("LOGBOOK_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_DB_PATH));

        let port = match env::var("LOGBOOK_PORT") {
            Ok(value) => value
                .parse::<u16>()
                .map_err(|_| ConfigError::InvalidPort(value))?,
            Err(_) => DEFAULT_PORT,
        };

        Ok(Self { db_path, port })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("invalid LOGBOOK_PORT value: {0}")]
    InvalidPort(String),
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError};
    use std::{env, sync::Mutex};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn uses_defaults_when_env_is_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        remove_var("LOGBOOK_DB_PATH");
        remove_var("LOGBOOK_PORT");

        let config = Config::from_env().unwrap();

        assert_eq!(config.db_path.as_os_str(), "/data/logbook.db");
        assert_eq!(config.port, 8080);
    }

    #[test]
    fn parses_env_overrides() {
        let _guard = ENV_LOCK.lock().unwrap();
        set_var("LOGBOOK_DB_PATH", "/tmp/logbook-test.db");
        set_var("LOGBOOK_PORT", "9090");

        let config = Config::from_env().unwrap();

        assert_eq!(config.db_path.as_os_str(), "/tmp/logbook-test.db");
        assert_eq!(config.port, 9090);

        remove_var("LOGBOOK_DB_PATH");
        remove_var("LOGBOOK_PORT");
    }

    #[test]
    fn rejects_invalid_port() {
        let _guard = ENV_LOCK.lock().unwrap();
        set_var("LOGBOOK_PORT", "nope");

        let error = Config::from_env().unwrap_err();

        assert_eq!(error, ConfigError::InvalidPort("nope".to_string()));
        remove_var("LOGBOOK_PORT");
    }

    fn set_var(key: &str, value: &str) {
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_var(key: &str) {
        unsafe {
            env::remove_var(key);
        }
    }
}
