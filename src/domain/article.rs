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
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct ArticleListResponse {
    pub items: Vec<Article>,
}

impl Article {
    pub fn as_card(&self) -> Article {
        Self {
            sections: Vec::new(),
            ..self.clone()
        }
    }
}
