use serde::Deserialize;
use once_cell::sync::Lazy;
use config::{Config, ConfigError, File};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server_addr: String,
    pub database_url: String,
    pub redis_url: Option<String>,
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
    pub allowed_origins: Vec<String>,
}

pub static SETTINGS: Lazy<Config> = Lazy::new(|| {
    Config::from_env().expect("Failed to load configuration")
});

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(config::Environment::with_prefix("IA").separator("__"))
            .build()?;

        config.try_deserialize()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_addr: "0.0.0.0:8080".to_string(),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://iainote:password@localhost:5432/iainote".to_string()),
            redis_url: std::env::var("REDIS_URL").ok(),
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev_secret_change_in_production".to_string()),
            jwt_expiry_hours: 24 * 7,
            allowed_origins: vec!["*".to_string()],
        }
    }
}
