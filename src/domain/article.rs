use serde::{Deserialize, Serialize};

use super::ContentSection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ArticleStatus {
    Draft,
    Published,
}

impl ArticleStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "draft" => Ok(Self::Draft),
            "published" => Ok(Self::Published),
            other => Err(format!("unknown article status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct Article {
    pub revision: i64,
    pub slug: String,
    pub title: String,
    pub category: String,
    pub summary: String,
    pub status: ArticleStatus,
    pub featured: bool,
    pub sort_order: i64,
    pub published_at: Option<String>,
    pub reading_time_minutes: Option<i64>,
    pub sections: Vec<ContentSection>,
    /// Canonical Publisle document JSON. Present on detail views only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<serde_json::Value>,
    /// Compiled publication artifact JSON rendered by the site. Present on
    /// detail views only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiled: Option<serde_json::Value>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct ArticleListResponse {
    pub items: Vec<Article>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

impl Article {
    pub fn as_card(&self) -> Article {
        Self {
            sections: Vec::new(),
            document: None,
            compiled: None,
            ..self.clone()
        }
    }
}
