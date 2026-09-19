use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::config::Config;
use crate::domain::{Article, ArticleStatus, MediaPlaceholder, Project};
use crate::error::AppError;

mod models;
mod seed;

#[derive(Clone)]
pub struct Db {
    inner: Arc<Mutex<toasty::Db>>,
}

impl Db {
    pub async fn connect(config: &Config) -> Result<Self, AppError> {
        Self::open(&config.database_url, &config.auth_token).await
    }

    pub async fn memory() -> Result<Self, AppError> {
        Self::open("turso::memory:", "").await
    }

    async fn open(url: &str, token: &str) -> Result<Self, AppError> {
        let db = connect_toasty(url, token).await?;
        Ok(Self {
            inner: Arc::new(Mutex::new(db)),
        })
    }

    pub async fn migrate(&self) -> Result<(), AppError> {
        let db = self.inner.lock().await;
        apply_schema(&db).await
    }

    pub async fn seed_if_empty(&self) -> Result<(), AppError> {
        let mut db = self.inner.lock().await;
        seed::seed_if_empty(&mut db).await
    }

    pub async fn ping(&self) -> Result<(), AppError> {
        let mut db = self.inner.lock().await;
        let _ = models::Project::all().first().exec(&mut *db).await?;
        Ok(())
    }

    pub async fn list_projects(&self, featured: Option<bool>) -> Result<Vec<Project>, AppError> {
        let mut db = self.inner.lock().await;
        let rows = if featured == Some(true) {
            models::Project::filter(models::Project::fields().featured().eq(true))
                .exec(&mut *db)
                .await?
        } else {
            models::Project::all().exec(&mut *db).await?
        };
        let mut items = rows
            .into_iter()
            .map(project_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        items.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then(a.slug.cmp(&b.slug)));
        Ok(items.into_iter().map(|item| item.as_card()).collect())
    }

    pub async fn get_project(&self, slug: &str) -> Result<Project, AppError> {
        let mut db = self.inner.lock().await;
        let row = models::Project::filter_by_slug(slug)
            .first()
            .exec(&mut *db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("project '{slug}' not found")))?;
        project_from_row(row)
    }

    pub async fn list_articles(&self, featured: Option<bool>) -> Result<Vec<Article>, AppError> {
        let mut db = self.inner.lock().await;
        let rows = if featured == Some(true) {
            models::Article::filter(models::Article::fields().featured().eq(true))
                .exec(&mut *db)
                .await?
        } else {
            models::Article::all().exec(&mut *db).await?
        };
        let mut items = rows
            .into_iter()
            .map(article_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        items.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then(a.slug.cmp(&b.slug)));
        Ok(items.into_iter().map(|item| item.as_card()).collect())
    }

    pub async fn get_article(&self, slug: &str) -> Result<Article, AppError> {
        let mut db = self.inner.lock().await;
        let row = models::Article::filter_by_slug(slug)
            .first()
            .exec(&mut *db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("article '{slug}' not found")))?;
        article_from_row(row)
    }

    pub async fn insert_contact(
        &self,
        name: &str,
        email: &str,
        message: &str,
    ) -> Result<(), AppError> {
        let mut db = self.inner.lock().await;
        toasty::create!(models::ContactMessage {
            name: name.to_owned(),
            email: email.to_owned(),
            message: message.to_owned(),
        })
        .exec(&mut *db)
        .await?;
        Ok(())
    }

    pub async fn subscribe_newsletter(&self, email: &str) -> Result<(), AppError> {
        let mut db = self.inner.lock().await;
        if models::NewsletterSubscriber::filter_by_email(email)
            .first()
            .exec(&mut *db)
            .await?
            .is_some()
        {
            return Ok(());
        }
        toasty::create!(models::NewsletterSubscriber {
            email: email.to_owned(),
        })
        .exec(&mut *db)
        .await?;
        Ok(())
    }
}

async fn apply_schema(db: &toasty::Db) -> Result<(), AppError> {
    match db.push_schema().await {
        Ok(()) => Ok(()),
        Err(err) if schema_already_exists(&err) => {
            tracing::info!("database schema already exists, skipping create");
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

fn schema_already_exists(err: &toasty::Error) -> bool {
    err.to_string().contains("already exists")
}

async fn connect_toasty(url: &str, token: &str) -> Result<toasty::Db, AppError> {
    if url == "memory" || url == "turso::memory:" {
        return Ok(toasty::Db::builder()
            .models(model_set())
            .connect("turso::memory:")
            .await?);
    }

    if url.starts_with("libsql://") || url.starts_with("https://") {
        let path =
            std::env::var("TURSO_LOCAL_PATH").unwrap_or_else(|_| "data/voidreg.db".to_owned());
        if let Some(parent) = Path::new(&path).parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        let mut driver = toasty_driver_turso::Turso::file(&path).with_remote_url(url);
        if !token.is_empty() {
            driver = driver.with_auth_token(token);
        }
        return Ok(toasty::Db::builder()
            .models(model_set())
            .build(driver)
            .await?);
    }

    let url = if url.starts_with("turso:") {
        url.to_owned()
    } else {
        format!("turso:{url}")
    };
    Ok(toasty::Db::builder()
        .models(model_set())
        .connect(&url)
        .await?)
}

fn model_set() -> toasty::ModelSet {
    toasty::models!(
        crate::db::models::Project,
        crate::db::models::Article,
        crate::db::models::ContactMessage,
        crate::db::models::NewsletterSubscriber
    )
}

fn project_from_row(row: models::Project) -> Result<Project, AppError> {
    Ok(Project {
        slug: row.slug,
        title: row.title,
        category: row.category,
        summary: row.summary,
        tags: serde_json::from_str(&row.tags)?,
        has_detail_page: row.has_detail_page,
        featured: row.featured,
        sort_order: row.sort_order,
        sections: serde_json::from_str(&row.sections)?,
        hero: row
            .hero
            .filter(|value| !value.is_empty())
            .map(|value| serde_json::from_str::<MediaPlaceholder>(&value))
            .transpose()?,
        related_slugs: serde_json::from_str(&row.related_slugs)?,
        repo_url: row.repo_url,
    })
}

fn article_from_row(row: models::Article) -> Result<Article, AppError> {
    Ok(Article {
        slug: row.slug,
        title: row.title,
        category: row.category,
        summary: row.summary,
        status: ArticleStatus::parse(&row.status).map_err(AppError::Internal)?,
        featured: row.featured,
        sort_order: row.sort_order,
        published_at: row.published_at,
        reading_time_minutes: row.reading_time_minutes,
        sections: serde_json::from_str(&row.sections)?,
    })
}
