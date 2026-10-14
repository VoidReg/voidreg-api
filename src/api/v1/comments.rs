use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;

use crate::auth::client_ip;
use crate::auth::extract::OptionalCommenter;
use crate::auth::rate::RateLimiter;
use crate::config::Config;
use crate::db::Db;
use crate::domain::{
    COMMENT_STATUS_APPROVED, COMMENT_STATUS_PENDING, CommentCreateRequest, CommentCreateResponse,
    CommentListResponse, CommentPublic,
};
use crate::error::AppError;
use crate::util::sha256_hex;

const MAX_NAME_LEN: usize = 80;
const MAX_BODY_LEN: usize = 2000;
const RATE_WINDOW_MINUTES: u64 = 60;
const PER_IP_MAX_COMMENTS: usize = 5;
const PER_IP_MAX_PER_ARTICLE: usize = 2;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::IntoParams))]
pub struct CommentListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/blogs/{slug}/comments",
    tag = "comments",
    params(
        ("slug" = String, Path, description = "Blog slug"),
        CommentListQuery
    ),
    responses(
        (status = 200, description = "Approved comments", body = CommentListResponse),
        (status = 404, description = "Blog not found", body = crate::error::ErrorBody)
    )
))]
#[get("/blogs/{slug}/comments")]
pub async fn list_comments(
    db: web::Data<Db>,
    path: web::Path<String>,
    query: web::Query<CommentListQuery>,
) -> Result<HttpResponse, AppError> {
    let slug = path.into_inner();
    // 404 when the article does not exist or is not published.
    db.get_published_article(&slug).await?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 50);
    let (comments, total) = db.list_public_comments(&slug, page, per_page).await?;
    let items = comments
        .into_iter()
        .map(|comment| CommentPublic {
            id: comment.id,
            author_name: comment.author_name,
            body: comment.body,
            created_at: comment.created_at,
            verified: comment.identity_id.is_some(),
        })
        .collect();
    Ok(HttpResponse::Ok().json(CommentListResponse {
        items,
        page,
        per_page,
        total,
    }))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/api/v1/blogs/{slug}/comments",
    tag = "comments",
    request_body = CommentCreateRequest,
    params(("slug" = String, Path, description = "Blog slug")),
    responses(
        (status = 202, description = "Comment accepted", body = CommentCreateResponse),
        (status = 404, description = "Blog not found", body = crate::error::ErrorBody),
        (status = 422, description = "Invalid payload", body = crate::error::ErrorBody),
        (status = 429, description = "Rate limited", body = crate::error::ErrorBody)
    )
))]
#[post("/blogs/{slug}/comments")]
pub async fn create_comment(
    db: web::Data<Db>,
    config: web::Data<Config>,
    limiter: web::Data<RateLimiter>,
    path: web::Path<String>,
    payload: web::Json<CommentCreateRequest>,
    commenter: OptionalCommenter,
    request: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let slug = path.into_inner();
    db.get_published_article(&slug).await?;

    let name = commenter
        .identity
        .as_ref()
        .map(|identity| identity.name.as_str())
        .unwrap_or(&payload.name)
        .trim();
    let body = payload.body.trim();

    if name.is_empty() || name.chars().count() > MAX_NAME_LEN {
        return Err(AppError::Unprocessable(format!(
            "name must be between 1 and {MAX_NAME_LEN} characters"
        )));
    }
    if body.is_empty() || body.chars().count() > MAX_BODY_LEN {
        return Err(AppError::Unprocessable(format!(
            "comment must be between 1 and {MAX_BODY_LEN} characters"
        )));
    }
    if let Some(email) = payload
        .email
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        && !crate::api::v1::contact::is_valid_email(email)
    {
        return Err(AppError::Unprocessable(
            "enter a valid email address".to_owned(),
        ));
    }

    // Honeypot: act accepted and drop the comment.
    if payload
        .website
        .as_deref()
        .map(str::trim)
        .is_some_and(|v| !v.is_empty())
    {
        return Ok(HttpResponse::Accepted().json(CommentCreateResponse {
            id: String::new(),
            status: COMMENT_STATUS_PENDING.to_owned(),
        }));
    }

    let ip_hash = comment_ip_hash(&config, &request);

    // Rate limits run before the write; verified identities are still
    // throttled to keep a compromised session from flooding.
    let window = std::time::Duration::from_secs(RATE_WINDOW_MINUTES * 60);
    if !limiter.check(&format!("comments:{ip_hash}"), PER_IP_MAX_COMMENTS, window) {
        return Err(AppError::TooManyRequests(
            "too many comments from this address; try again later".to_owned(),
        ));
    }
    if !limiter.check(
        &format!("comments:{ip_hash}:{slug}"),
        PER_IP_MAX_PER_ARTICLE,
        window,
    ) {
        return Err(AppError::TooManyRequests(
            "too many comments on this article from this address; try again later".to_owned(),
        ));
    }

    // Verified commenters skip moderation; anonymous comments wait for
    // approval in the admin queue.
    let status = if commenter.identity.is_some() {
        COMMENT_STATUS_APPROVED
    } else {
        COMMENT_STATUS_PENDING
    };
    let identity_id = commenter
        .identity
        .as_ref()
        .map(|identity| identity.id.clone());

    let comment = db
        .insert_comment(&slug, identity_id.as_deref(), name, body, status, &ip_hash)
        .await?;

    Ok(HttpResponse::Accepted().json(CommentCreateResponse {
        id: comment.id,
        status: status.to_owned(),
    }))
}

/// Salted hash of the client IP; raw addresses are never stored.
fn comment_ip_hash(config: &Config, request: &HttpRequest) -> String {
    sha256_hex(format!("{}:{}", config.comment_ip_salt, client_ip(request)).as_bytes())
}
