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
        (status = 200, description = "Visible discussion threads", body = CommentListResponse),
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
    let comments = db.public_discussion(&slug).await?;
    let roots: Vec<_> = comments
        .iter()
        .rev()
        .filter(|c| c.parent_id.is_none())
        .collect();
    let total = roots.len() as i64;
    let start = page.saturating_sub(1).saturating_mul(per_page) as usize;
    let items = roots
        .into_iter()
        .skip(start)
        .take(per_page as usize)
        .map(|comment| public_comment(comment, &comments))
        .collect();
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(CommentListResponse {
            items,
            page,
            per_page,
            total,
        }))
}

fn public_comment(
    comment: &crate::db::CommentInfo,
    all: &[crate::db::CommentInfo],
) -> CommentPublic {
    CommentPublic {
        id: comment.id.clone(),
        parent_id: comment.parent_id.clone(),
        thread_id: comment.thread_id.clone(),
        replying_to: comment
            .parent_id
            .as_ref()
            .and_then(|id| all.iter().find(|c| &c.id == id))
            .map(|c| c.author_name.clone()),
        reply_count: all
            .iter()
            .filter(|c| c.thread_id.as_deref() == Some(&comment.id))
            .count() as i64,
        removed: comment.status == "removed",
        author_name: comment.author_name.clone(),
        body: comment.body.clone(),
        created_at: comment.created_at.clone(),
        verified: comment.identity_id.is_some(),
    }
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get, path = "/api/v1/blogs/{slug}/comments/{id}/replies", tag = "comments",
    params(("slug" = String, Path), ("id" = String, Path), CommentListQuery),
    responses((status = 200, description = "Replies in chronological order", body = CommentListResponse))
))]
#[get("/blogs/{slug}/comments/{id}/replies")]
pub async fn list_replies(
    db: web::Data<Db>,
    path: web::Path<(String, String)>,
    query: web::Query<CommentListQuery>,
) -> Result<HttpResponse, AppError> {
    let (slug, id) = path.into_inner();
    db.get_published_article(&slug).await?;
    let comments = db.public_discussion(&slug).await?;
    let root = comments
        .iter()
        .find(|c| c.id == id && c.parent_id.is_none())
        .ok_or_else(|| AppError::NotFound("discussion not found".to_owned()))?;
    let replies: Vec<_> = comments
        .iter()
        .filter(|c| c.thread_id.as_deref() == Some(&root.id))
        .collect();
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 50);
    let total = replies.len() as i64;
    let start = page.saturating_sub(1).saturating_mul(per_page) as usize;
    let items = replies
        .into_iter()
        .skip(start)
        .take(per_page as usize)
        .map(|c| public_comment(c, &comments))
        .collect();
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(CommentListResponse {
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

    let parent = if let Some(id) = &payload.parent_id {
        let parent = db
            .get_comment(id)
            .await?
            .filter(|c| c.article_slug == slug && c.status == "approved")
            .ok_or_else(|| AppError::Unprocessable("reply target is unavailable".to_owned()))?;
        Some(parent)
    } else {
        None
    };
    let thread_id = parent
        .as_ref()
        .map(|p| p.thread_id.as_deref().unwrap_or(&p.id));
    let ip_hash = comment_ip_hash(&config, &request);

    // Rate limits run before the write; verified identities are still
    // throttled to keep a compromised session from flooding.
    let window = std::time::Duration::from_secs(RATE_WINDOW_MINUTES * 60);
    if !limiter.check(
        &format!("comments:{ip_hash}"),
        if commenter.identity.is_some() {
            60
        } else {
            PER_IP_MAX_COMMENTS
        },
        window,
    ) {
        return Err(AppError::TooManyRequests(
            "too many comments from this address; try again later".to_owned(),
        ));
    }
    if !limiter.check(
        &format!("comments:{ip_hash}:{slug}"),
        if commenter.identity.is_some() {
            30
        } else {
            PER_IP_MAX_PER_ARTICLE
        },
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
        .insert_comment(crate::db::NewComment {
            article_slug: &slug,
            identity_id: identity_id.as_deref(),
            author_name: name,
            body,
            status,
            ip_hash: &ip_hash,
            parent_id: payload.parent_id.as_deref(),
            thread_id,
        })
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
