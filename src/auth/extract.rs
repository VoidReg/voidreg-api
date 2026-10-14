//! Request extractors for authenticated admin and commenter sessions.

use std::future::{Ready, ready};
use std::pin::Pin;

use actix_web::dev::Payload;
use actix_web::web::Data;
use actix_web::{FromRequest, HttpRequest};

use crate::auth::{
    ADMIN_SESSION_COOKIE, COMMENTER_SESSION_COOKIE, SESSION_KIND_ADMIN, SESSION_KIND_COMMENTER,
    cookie_token, csrf_check, hash_token,
};
use crate::db::Db;
use crate::domain::{CommenterIdentity, CommenterSessionResponse};
use crate::error::AppError;

/// Extracts a live admin session cookie or fails the request with 401.
/// Handlers call `validate` to check the session against the database, and
/// unsafe methods pass the CSRF double-submit check here.
pub struct AdminSession {
    pub token: String,
}

impl FromRequest for AdminSession {
    type Error = AppError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(request: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(from_request_admin(request))
    }
}

fn from_request_admin(request: &HttpRequest) -> Result<AdminSession, AppError> {
    let token = cookie_token(request, ADMIN_SESSION_COOKIE)
        .ok_or_else(|| AppError::Unauthorized("admin authentication required".to_owned()))?;
    csrf_check(request)?;
    Ok(AdminSession { token })
}

impl AdminSession {
    /// Verifies the session token against the database.
    pub async fn validate(&self, db: &Db) -> Result<(), AppError> {
        let session = db
            .get_live_session(&hash_token(&self.token))
            .await?
            .ok_or_else(|| AppError::Unauthorized("admin session expired".to_owned()))?;
        if session.kind != SESSION_KIND_ADMIN {
            return Err(AppError::Unauthorized("admin session required".to_owned()));
        }
        Ok(())
    }
}

/// Extracts the commenter identity behind a valid commenter session, or None
/// for anonymous visitors. Never fails the request.
pub struct OptionalCommenter {
    pub identity: Option<CommenterIdentity>,
}

impl FromRequest for OptionalCommenter {
    type Error = AppError;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(request: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let db = request
            .app_data::<Data<Db>>()
            .map(|data| data.get_ref().clone());
        let token = cookie_token(request, COMMENTER_SESSION_COOKIE);
        Box::pin(async move {
            let Some(db) = db else {
                return Ok(OptionalCommenter { identity: None });
            };
            let Some(token) = token else {
                return Ok(OptionalCommenter { identity: None });
            };
            let Some(session) = db.get_live_session(&hash_token(&token)).await? else {
                return Ok(OptionalCommenter { identity: None });
            };
            if session.kind != SESSION_KIND_COMMENTER {
                return Ok(OptionalCommenter { identity: None });
            }
            let Some(identity_id) = session.identity_id else {
                return Ok(OptionalCommenter { identity: None });
            };
            let identity = db.get_identity_by_id(&identity_id).await?;
            Ok(OptionalCommenter { identity })
        })
    }
}

/// Convenience conversion used by `GET /api/v1/auth/session`.
impl From<Option<CommenterIdentity>> for CommenterSessionResponse {
    fn from(identity: Option<CommenterIdentity>) -> Self {
        Self {
            authenticated: identity.is_some(),
            identity,
        }
    }
}
