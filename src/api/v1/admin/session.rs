use actix_web::{HttpRequest, HttpResponse, delete, get, post, web};

use crate::auth::extract::AdminSession;
use crate::auth::rate::RateLimiter;
use crate::auth::{
    SESSION_KIND_ADMIN, admin_csrf_cookie, admin_session_cookie, client_ip, expired_cookie,
    hash_token, new_session_token, verify_admin_password,
};
use crate::config::Config;
use crate::db::Db;
use crate::domain::{AdminLoginRequest, AdminSessionResponse};
use crate::error::AppError;

const LOGIN_WINDOW_MINUTES: u64 = 15;
const LOGIN_MAX_ATTEMPTS: usize = 10;

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/api/v1/admin/session",
    tag = "admin",
    request_body = AdminLoginRequest,
    responses(
        (status = 200, description = "Signed in; session cookies set", body = AdminSessionResponse),
        (status = 401, description = "Invalid password", body = crate::error::ErrorBody),
        (status = 429, description = "Too many attempts", body = crate::error::ErrorBody),
        (status = 503, description = "Admin auth not configured", body = crate::error::ErrorBody)
    )
))]
#[post("/session")]
pub async fn login(
    db: web::Data<Db>,
    config: web::Data<Config>,
    limiter: web::Data<RateLimiter>,
    payload: web::Json<AdminLoginRequest>,
    request: HttpRequest,
) -> Result<HttpResponse, AppError> {
    if !config.admin_auth_configured() {
        return Err(AppError::ServiceUnavailable(
            "admin authentication is not configured; set ADMIN_PASSWORD or ADMIN_PASSWORD_HASH"
                .to_owned(),
        ));
    }

    let ip = client_ip(&request);
    let window = std::time::Duration::from_secs(LOGIN_WINDOW_MINUTES * 60);
    if !limiter.check(&format!("admin-login:{ip}"), LOGIN_MAX_ATTEMPTS, window) {
        return Err(AppError::TooManyRequests(
            "too many sign-in attempts; try again later".to_owned(),
        ));
    }

    if !verify_admin_password(&config, &payload.password).await? {
        return Err(AppError::Unauthorized("invalid admin password".to_owned()));
    }

    let (token, token_hash) = new_session_token();
    db.create_session(
        SESSION_KIND_ADMIN,
        &token_hash,
        None,
        config.admin_session_ttl_hours,
    )
    .await?;

    // The CSRF value is derived from the session token, so the header check
    // is bound to this session without extra storage.
    let csrf = csrf_for(&token);

    Ok(HttpResponse::Ok()
        .cookie(admin_session_cookie(&config, &token))
        .cookie(admin_csrf_cookie(&config, &csrf))
        .json(AdminSessionResponse {
            authenticated: true,
        }))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/admin/session",
    tag = "admin",
    responses(
        (status = 200, description = "Session is valid", body = AdminSessionResponse),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[get("/session")]
pub async fn show(db: web::Data<Db>, session: AdminSession) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;
    Ok(HttpResponse::Ok().json(AdminSessionResponse {
        authenticated: true,
    }))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    delete,
    path = "/api/v1/admin/session",
    tag = "admin",
    responses(
        (status = 200, description = "Signed out", body = AdminSessionResponse),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[delete("/session")]
pub async fn logout(
    db: web::Data<Db>,
    config: web::Data<Config>,
    session: AdminSession,
) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;
    db.delete_session(&hash_token(&session.token)).await?;
    Ok(HttpResponse::Ok()
        .cookie(expired_cookie(crate::auth::ADMIN_SESSION_COOKIE, &config))
        .cookie(expired_cookie(crate::auth::ADMIN_CSRF_COOKIE, &config))
        .json(AdminSessionResponse {
            authenticated: false,
        }))
}

/// CSRF value bound to an admin session token.
pub fn csrf_for(token: &str) -> String {
    hash_token(&format!("csrf:{token}"))
}
