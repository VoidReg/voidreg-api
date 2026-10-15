use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct CommenterIdentity {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct CommenterSessionResponse {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<CommenterIdentity>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct AdminSessionResponse {
    pub authenticated: bool,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct AdminLoginRequest {
    pub password: String,
}

/// Body for creating or updating an article through the admin API.
///
/// On create, `title`, `category`, and `summary` are required. On update,
/// every field is optional and absent fields keep their current value.
/// `slug` is only honored on create; slugs are immutable afterwards.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct ArticleWriteRequest {
    pub expected_revision: Option<i64>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub featured: Option<bool>,
    #[serde(default)]
    pub sort_order: Option<i64>,
    #[serde(default)]
    pub reading_time_minutes: Option<i64>,
    /// Canonical Publisle document. Replaces any stored document when present.
    #[serde(default)]
    pub document: Option<serde_json::Value>,
    /// Compiled publication artifact from @publisle/adapter-core.
    #[serde(default)]
    pub compiled: Option<serde_json::Value>,
    /// Publisle document schema version. Required whenever a document is set.
    #[serde(default)]
    pub document_version: Option<i64>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct ArticleMutationResponse {
    pub revision: i64,
    pub slug: String,
    pub status: String,
    pub content_hash: String,
}
