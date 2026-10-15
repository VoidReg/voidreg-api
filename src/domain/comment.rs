use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct CommentCreateResponse {
    pub id: String,
    /// "pending" while awaiting moderation, "approved" when visible.
    pub status: String,
}

/// Public comment as shown under an article.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct CommentPublic {
    pub parent_id: Option<String>,
    pub thread_id: Option<String>,
    pub replying_to: Option<String>,
    pub reply_count: i64,
    pub removed: bool,
    pub id: String,
    pub author_name: String,
    pub body: String,
    pub created_at: String,
    /// True when the comment is tied to a verified OAuth identity.
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct CommentListResponse {
    pub items: Vec<CommentPublic>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct CommentCreateRequest {
    #[serde(default)]
    pub parent_id: Option<String>,
    pub name: String,
    pub body: String,
    /// Optional legacy input, validated but not retained.
    #[serde(default)]
    pub email: Option<String>,
    /// Honeypot field. Must stay empty; bots that fill it are silently dropped.
    #[serde(default)]
    pub website: Option<String>,
}

/// Admin view of a comment, including moderation state and contact email.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct CommentAdminView {
    pub parent_id: Option<String>,
    pub thread_id: Option<String>,
    pub parent_author: Option<String>,
    pub parent_body: Option<String>,
    pub id: String,
    pub article_slug: String,
    pub author_name: String,
    pub body: String,
    pub status: String,
    pub created_at: String,
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct CommentAdminListResponse {
    pub items: Vec<CommentAdminView>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

pub const COMMENT_STATUS_PENDING: &str = "pending";
pub const COMMENT_STATUS_APPROVED: &str = "approved";
