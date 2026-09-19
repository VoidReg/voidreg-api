use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct ContentSection {
    pub heading: Option<String>,
    pub body: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct MediaPlaceholder {
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct Project {
    pub slug: String,
    pub title: String,
    pub category: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub has_detail_page: bool,
    pub featured: bool,
    pub sort_order: i64,
    pub sections: Vec<ContentSection>,
    pub hero: Option<MediaPlaceholder>,
    pub related_slugs: Vec<String>,
    pub repo_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct ProjectListResponse {
    pub items: Vec<Project>,
}

impl Project {
    pub fn as_card(&self) -> Project {
        Self {
            sections: Vec::new(),
            hero: None,
            related_slugs: Vec::new(),
            repo_url: None,
            ..self.clone()
        }
    }
}
