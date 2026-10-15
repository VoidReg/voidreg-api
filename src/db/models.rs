#[derive(Debug, toasty::Model)]
pub struct Project {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[unique]
    pub slug: String,
    pub title: String,
    pub category: String,
    pub summary: String,
    pub tags: String,
    pub sections: String,
    pub hero: Option<String>,
    pub has_detail_page: bool,
    pub featured: bool,
    pub sort_order: i64,
    pub repo_url: Option<String>,
    pub related_slugs: String,
}

#[derive(Debug, toasty::Model)]
pub struct Article {
    pub revision: i64,
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[unique]
    pub slug: String,
    pub title: String,
    pub category: String,
    pub summary: String,
    pub status: String,
    pub featured: bool,
    pub sort_order: i64,
    pub published_at: Option<String>,
    pub reading_time_minutes: Option<i64>,
    pub sections: String,
    pub document: String,
    pub document_version: i64,
    pub compiled: String,
    pub content_hash: String,
}

#[derive(Debug, toasty::Model)]
pub struct ContactMessage {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    pub name: String,
    pub email: String,
    pub message: String,
}

#[derive(Debug, toasty::Model)]
pub struct NewsletterSubscriber {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[unique]
    pub email: String,
}

#[derive(Debug, toasty::Model)]
pub struct Comment {
    pub parent_id: Option<String>,
    pub thread_id: Option<String>,
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    pub article_slug: String,
    pub identity_id: Option<String>,
    pub author_name: String,
    pub body: String,
    pub status: String,
    pub ip_hash: String,
    pub created_at: String,
}

#[derive(Debug, toasty::Model)]
pub struct Identity {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    pub provider: String,
    pub subject: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, toasty::Model)]
pub struct Session {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[unique]
    pub token_hash: String,
    pub kind: String,
    pub identity_id: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, toasty::Model)]
pub struct McpToken {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[unique]
    pub token_hash: String,
    pub identity_id: String,
    pub name: String,
    pub created_at: String,
    pub expires_at: String,
    pub last_used_at: Option<String>,
}
