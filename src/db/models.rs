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
