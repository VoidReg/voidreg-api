use actix_web::{HttpResponse, delete, get, post, web};
use serde::{Deserialize, Serialize};

use crate::auth::extract::AdminSession;
use crate::db::Db;
use crate::domain::{COMMENT_STATUS_APPROVED, CommentAdminListResponse, CommentAdminView};
use crate::error::AppError;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::IntoParams))]
pub struct AdminCommentListQuery {
    /// Filter by status: "pending", "approved", or omit for all.
    pub status: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct CommentMutationResponse {
    pub id: String,
    pub status: String,
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/admin/comments",
    tag = "admin",
    params(AdminCommentListQuery),
    responses(
        (status = 200, description = "Comments for moderation", body = CommentAdminListResponse),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[get("/comments")]
pub async fn list_comments(
    db: web::Data<Db>,
    query: web::Query<AdminCommentListQuery>,
    session: AdminSession,
) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;

    let status = match query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some("pending") => Some("pending"),
        Some("approved") => Some("approved"),
        Some(other) => {
            return Err(AppError::Unprocessable(format!(
                "unknown comment status filter '{other}'"
            )));
        }
        None => None,
    };
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let (comments, total) = db.list_admin_comments(status, page, per_page).await?;
    let mut items = Vec::with_capacity(comments.len());
    for comment in comments {
        let email = match &comment.identity_id {
            Some(identity_id) => db
                .get_identity_by_id(identity_id)
                .await?
                .map(|identity| identity.email),
            None => None,
        };
        let parent = match &comment.parent_id {
            Some(id) => db.get_comment(id).await?,
            None => None,
        };
        items.push(CommentAdminView {
            parent_id: comment.parent_id.clone(),
            thread_id: comment.thread_id.clone(),
            parent_author: parent.as_ref().map(|c| c.author_name.clone()),
            parent_body: parent.map(|c| c.body),
            id: comment.id,
            article_slug: comment.article_slug,
            author_name: comment.author_name,
            body: comment.body,
            status: comment.status,
            created_at: comment.created_at,
            verified: comment.identity_id.is_some(),
            email,
        });
    }

    Ok(HttpResponse::Ok().json(CommentAdminListResponse {
        items,
        page,
        per_page,
        total,
    }))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/api/v1/admin/comments/{id}/approve",
    tag = "admin",
    params(("id" = String, Path, description = "Comment id")),
    responses(
        (status = 200, description = "Comment approved", body = CommentMutationResponse),
        (status = 404, description = "Comment not found", body = crate::error::ErrorBody),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[post("/comments/{id}/approve")]
pub async fn approve_comment(
    db: web::Data<Db>,
    path: web::Path<String>,
    session: AdminSession,
) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;
    let id = path.into_inner();
    let comment = db
        .get_comment(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("comment '{id}' not found")))?;
    if comment.status == "removed" {
        return Err(AppError::Unprocessable(
            "removed comments cannot be approved".to_owned(),
        ));
    }
    db.approve_comment(&id).await?;
    Ok(HttpResponse::Ok().json(CommentMutationResponse {
        id: comment.id,
        status: COMMENT_STATUS_APPROVED.to_owned(),
    }))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    delete,
    path = "/api/v1/admin/comments/{id}",
    tag = "admin",
    params(("id" = String, Path, description = "Comment id")),
    responses(
        (status = 200, description = "Comment deleted"),
        (status = 404, description = "Comment not found", body = crate::error::ErrorBody),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[delete("/comments/{id}")]
pub async fn delete_comment(
    db: web::Data<Db>,
    path: web::Path<String>,
    session: AdminSession,
) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;
    let id = path.into_inner();
    db.get_comment(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("comment '{id}' not found")))?;
    db.delete_comment(&id).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "deleted": id })))
}
