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
    pub admin_password: Option<String>,
    pub admin_password_hash: Option<String>,
    pub admin_session_ttl_hours: i64,
    pub public_base_url: String,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub commenter_redirect_url: Option<String>,
    pub comment_ip_salt: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{0}")]
    Missing(&'static str),
    #[error("invalid PORT value")]
    InvalidPort,
    #[error("invalid ADMIN_SESSION_TTL_HOURS value")]
    InvalidSessionTtl,
}

fn optional_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
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
        let admin_session_ttl_hours = optional_env("ADMIN_SESSION_TTL_HOURS")
            .map(|value| {
                value
                    .parse::<i64>()
                    .map_err(|_| ConfigError::InvalidSessionTtl)
            })
            .transpose()?
            .unwrap_or(72);
        if admin_session_ttl_hours <= 0 {
            return Err(ConfigError::InvalidSessionTtl);
        }

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
            admin_password: optional_env("ADMIN_PASSWORD"),
            admin_password_hash: optional_env("ADMIN_PASSWORD_HASH"),
            admin_session_ttl_hours,
            public_base_url: optional_env("PUBLIC_BASE_URL")
                .unwrap_or_else(|| "http://127.0.0.1:8080".to_owned()),
            google_client_id: optional_env("GOOGLE_CLIENT_ID"),
            google_client_secret: optional_env("GOOGLE_CLIENT_SECRET"),
            github_client_id: optional_env("GITHUB_CLIENT_ID"),
            github_client_secret: optional_env("GITHUB_CLIENT_SECRET"),
            commenter_redirect_url: optional_env("COMMENTER_REDIRECT_URL"),
            comment_ip_salt: optional_env("COMMENT_IP_SALT").unwrap_or_default(),
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
            admin_password: Some("voidreg-test-admin".to_owned()),
            admin_password_hash: None,
            admin_session_ttl_hours: 72,
            public_base_url: "http://127.0.0.1:8080".to_owned(),
            google_client_id: None,
            google_client_secret: None,
            github_client_id: None,
            github_client_secret: None,
            commenter_redirect_url: None,
            comment_ip_salt: "test-comment-salt".to_owned(),
        }
    }

    pub fn skips_outbound_mail(&self) -> bool {
        self.resend_api_key.is_empty() || self.resend_api_key == "re_test"
    }

    pub fn resend_from(&self) -> String {
        format!("Voidreg <{}@{}>", self.resend_from_user, self.resend_domain)
    }

    pub fn admin_auth_configured(&self) -> bool {
        self.admin_password.is_some() || self.admin_password_hash.is_some()
    }

    pub fn oauth_provider(&self, provider: &str) -> Option<(&str, &str)> {
        match provider {
            "google" => Some((
                self.google_client_id.as_deref()?,
                self.google_client_secret.as_deref()?,
            )),
            "github" => Some((
                self.github_client_id.as_deref()?,
                self.github_client_secret.as_deref()?,
            )),
            _ => None,
        }
    }

    pub fn commenter_redirect(&self) -> String {
        self.commenter_redirect_url
            .clone()
            .unwrap_or_else(|| format!("{}/blog", self.public_base_url))
    }

    pub fn oauth_redirect_uri(&self, provider: &str) -> String {
        format!("{}/api/v1/auth/{provider}/callback", self.public_base_url)
    }
}
