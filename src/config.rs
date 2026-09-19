use std::env;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub auth_token: String,
    pub cors_origins: Vec<String>,
    pub resend_api_key: String,
    pub resend_domain: String,
    pub resend_from_user: String,
    pub contact_to_email: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{0}")]
    Missing(&'static str),
    #[error("invalid PORT value")]
    InvalidPort,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_owned());
        let port = env::var("PORT")
            .ok()
            .map(|value| value.parse::<u16>().map_err(|_| ConfigError::InvalidPort))
            .transpose()?
            .unwrap_or(8080);
        let database_url = env::var("TURSO_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .map_err(|_| ConfigError::Missing("TURSO_DATABASE_URL"))?;
        let auth_token = env::var("TURSO_AUTH_TOKEN").unwrap_or_default();
        let cors_origins = env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000,http://localhost:5173".to_owned())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        let resend_api_key =
            env::var("RESEND_API_KEY").map_err(|_| ConfigError::Missing("RESEND_API_KEY"))?;
        let resend_domain =
            env::var("RESEND_DOMAIN").map_err(|_| ConfigError::Missing("RESEND_DOMAIN"))?;
        let resend_from_user =
            env::var("RESEND_FROM_USER").map_err(|_| ConfigError::Missing("RESEND_FROM_USER"))?;
        let contact_to_email =
            env::var("CONTACT_TO_EMAIL").map_err(|_| ConfigError::Missing("CONTACT_TO_EMAIL"))?;

        Ok(Self {
            host,
            port,
            database_url,
            auth_token,
            cors_origins,
            resend_api_key,
            resend_domain,
            resend_from_user,
            contact_to_email,
        })
    }

    pub fn for_test() -> Self {
        Self {
            host: "127.0.0.1".to_owned(),
            port: 8080,
            database_url: "memory".to_owned(),
            auth_token: String::new(),
            cors_origins: vec!["http://localhost:5173".to_owned()],
            resend_api_key: "re_test".to_owned(),
            resend_domain: "mail.example.com".to_owned(),
            resend_from_user: "form".to_owned(),
            contact_to_email: "admin@example.com".to_owned(),
        }
    }

    pub fn skips_outbound_mail(&self) -> bool {
        self.resend_api_key.is_empty() || self.resend_api_key == "re_test"
    }

    pub fn resend_from(&self) -> String {
        format!("Voidreg <{}@{}>", self.resend_from_user, self.resend_domain)
    }
}
