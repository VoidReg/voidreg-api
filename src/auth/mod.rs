//! Admin and commenter authentication: password verification, cookie-backed
//! sessions, CSRF double-submit, and request extractors.

pub mod extract;
pub mod rate;

use actix_web::cookie::time::Duration as CookieDuration;
use actix_web::cookie::{Cookie, SameSite};
use argon2::PasswordVerifier;

use crate::config::Config;
use crate::error::AppError;
use crate::util::{constant_time_eq, random_token, sha256_hex};

pub const ADMIN_SESSION_COOKIE: &str = "voidreg_admin_session";
pub const ADMIN_CSRF_COOKIE: &str = "voidreg_admin_csrf";
pub const COMMENTER_SESSION_COOKIE: &str = "voidreg_commenter_session";
pub const OAUTH_STATE_COOKIE: &str = "voidreg_oauth_state";
pub const OAUTH_VERIFIER_COOKIE: &str = "voidreg_oauth_verifier";
pub const CSRF_HEADER: &str = "X-CSRF-Token";

pub const SESSION_KIND_ADMIN: &str = "admin";
pub const SESSION_KIND_COMMENTER: &str = "commenter";

pub const OAUTH_STATE_TTL_MINUTES: i64 = 10;

/// Verifies a candidate admin password against the configured secret.
/// Prefers the argon2 PHC hash when configured; falls back to a
/// constant-time comparison with the plain dev password.
pub async fn verify_admin_password(config: &Config, password: &str) -> Result<bool, AppError> {
    if let Some(hash) = &config.admin_password_hash {
        let hash = hash.clone();
        let password = password.to_owned();
        tokio::task::spawn_blocking(move || {
            let parsed = argon2::PasswordHash::new(&hash)
                .map_err(|err| AppError::Internal(format!("invalid ADMIN_PASSWORD_HASH: {err}")))?;
            Ok(argon2::Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok())
        })
        .await
        .map_err(|err| AppError::Internal(err.to_string()))?
    } else if let Some(expected) = &config.admin_password {
        Ok(constant_time_eq(expected.as_bytes(), password.as_bytes()))
    } else {
        Ok(false)
    }
}

/// Generates a fresh session token and returns it together with the hash to
/// persist. Only the hash is stored server-side.
pub fn new_session_token() -> (String, String) {
    let token = random_token();
    let token_hash = sha256_hex(token.as_bytes());
    (token, token_hash)
}

pub fn hash_token(token: &str) -> String {
    sha256_hex(token.as_bytes())
}

fn secure_cookies(config: &Config) -> bool {
    config.public_base_url.starts_with("https://")
}

/// Builds the HttpOnly admin session cookie.
pub fn admin_session_cookie(config: &Config, token: &str) -> Cookie<'static> {
    Cookie::build(ADMIN_SESSION_COOKIE, token.to_owned())
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure_cookies(config))
        .path("/")
        .max_age(CookieDuration::hours(config.admin_session_ttl_hours))
        .finish()
}

/// Builds the readable CSRF token cookie for the double-submit check.
pub fn admin_csrf_cookie(config: &Config, token: &str) -> Cookie<'static> {
    Cookie::build(ADMIN_CSRF_COOKIE, token.to_owned())
        .http_only(false)
        .same_site(SameSite::Lax)
        .secure(secure_cookies(config))
        .path("/")
        .max_age(CookieDuration::hours(config.admin_session_ttl_hours))
        .finish()
}

pub fn expired_cookie(name: &str, config: &Config) -> Cookie<'static> {
    Cookie::build(name.to_owned(), "")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure_cookies(config))
        .path("/")
        .max_age(CookieDuration::ZERO)
        .finish()
}

pub fn commenter_session_cookie(config: &Config, token: &str) -> Cookie<'static> {
    Cookie::build(COMMENTER_SESSION_COOKIE, token.to_owned())
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure_cookies(config))
        .path("/api/v1")
        .max_age(CookieDuration::hours(config.admin_session_ttl_hours))
        .finish()
}

pub fn commenter_expired_cookie(config: &Config) -> Cookie<'static> {
    Cookie::build(COMMENTER_SESSION_COOKIE, "")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure_cookies(config))
        .path("/api/v1")
        .max_age(CookieDuration::ZERO)
        .finish()
}

pub fn oauth_state_cookie(config: &Config, state: &str) -> Cookie<'static> {
    Cookie::build(OAUTH_STATE_COOKIE, state.to_owned())
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure_cookies(config))
        .path("/api/v1/auth")
        .max_age(CookieDuration::minutes(OAUTH_STATE_TTL_MINUTES))
        .finish()
}

pub fn oauth_verifier_cookie(config: &Config, verifier: &str) -> Cookie<'static> {
    Cookie::build(OAUTH_VERIFIER_COOKIE, verifier.to_owned())
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure_cookies(config))
        .path("/api/v1/auth")
        .max_age(CookieDuration::minutes(OAUTH_STATE_TTL_MINUTES))
        .finish()
}

pub fn expired_oauth_cookies(config: &Config) -> Vec<Cookie<'static>> {
    vec![
        Cookie::build(OAUTH_STATE_COOKIE, "")
            .http_only(true)
            .same_site(SameSite::Lax)
            .secure(secure_cookies(config))
            .path("/api/v1/auth")
            .max_age(CookieDuration::ZERO)
            .finish(),
        Cookie::build(OAUTH_VERIFIER_COOKIE, "")
            .http_only(true)
            .same_site(SameSite::Lax)
            .secure(secure_cookies(config))
            .path("/api/v1/auth")
            .max_age(CookieDuration::ZERO)
            .finish(),
    ]
}

/// Extracts the raw session token from a request cookie, if any.
pub fn cookie_token(request: &actix_web::HttpRequest, name: &str) -> Option<String> {
    request
        .cookie(name)
        .map(|cookie| cookie.value().to_owned())
        .filter(|value| !value.is_empty())
}

/// Best-effort client IP for rate limiting: honors the final X-Forwarded-For
/// hop (overwritten by the trusted edge nginx) and falls back to the peer address.
pub fn client_ip(request: &actix_web::HttpRequest) -> String {
    if let Some(forwarded) = request
        .headers()
        .get("X-Forwarded-For")
        .and_then(|value| value.to_str().ok())
        && let Some(first) = forwarded.split(',').next_back()
    {
        let ip = first.trim();
        if let Ok(address) = ip.parse::<std::net::IpAddr>() {
            return address.to_string();
        }
    }
    request
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// Resolves the per-request CSRF check: on unsafe methods the CSRF header
/// must match the value derived from the admin session token, binding the
/// token to this exact session.
pub fn csrf_check(request: &actix_web::HttpRequest) -> Result<(), AppError> {
    let method = request.method();
    if method == actix_web::http::Method::GET
        || method == actix_web::http::Method::HEAD
        || method == actix_web::http::Method::OPTIONS
    {
        return Ok(());
    }
    let session_token = cookie_token(request, ADMIN_SESSION_COOKIE)
        .ok_or_else(|| AppError::Unauthorized("admin authentication required".to_owned()))?;
    let expected = hash_token(&format!("csrf:{session_token}"));
    let header_token = request
        .headers()
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if constant_time_eq(expected.as_bytes(), header_token.as_bytes()) {
        Ok(())
    } else {
        Err(AppError::Forbidden("CSRF token mismatch".to_owned()))
    }
}
