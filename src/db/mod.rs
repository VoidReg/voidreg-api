use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::config::Config;
use crate::domain::{Article, ArticleStatus, MediaPlaceholder, Project};
use crate::error::AppError;
use crate::util::{now_rfc3339, sha256_hex};

mod models;
mod seed;

#[derive(Clone)]
pub struct Db {
    inner: Arc<Mutex<toasty::Db>>,
}

/// Full set of stored fields for an article, used by insert and save.
#[derive(Debug, Clone)]
pub struct ArticleFields {
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

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub kind: String,
    pub identity_id: Option<String>,
    pub expires_at: String,
}

#[derive(Debug, Clone)]
pub struct IdentityInfo {
    pub id: String,
    pub provider: String,
    pub subject: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommentInfo {
    pub parent_id: Option<String>,
    pub thread_id: Option<String>,
    pub id: String,
    pub article_slug: String,
    pub identity_id: Option<String>,
    pub author_name: String,
    pub body: String,
    pub status: String,
    pub created_at: String,
    pub email: Option<String>,
}

pub struct NewComment<'a> {
    pub article_slug: &'a str,
    pub identity_id: Option<&'a str>,
    pub author_name: &'a str,
    pub body: &'a str,
    pub status: &'a str,
    pub ip_hash: &'a str,
    pub parent_id: Option<&'a str>,
    pub thread_id: Option<&'a str>,
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
        let mut db = self.inner.lock().await;
        apply_schema(&mut db).await
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

    /// Published articles for the public blog index, newest first.
    pub async fn list_published_articles(
        &self,
        featured: Option<bool>,
    ) -> Result<Vec<Article>, AppError> {
        let mut db = self.inner.lock().await;
        let rows = if featured == Some(true) {
            models::Article::filter(toasty::stmt::Expr::and(
                models::Article::fields()
                    .status()
                    .eq(ArticleStatus::Published.as_str()),
                models::Article::fields().featured().eq(true),
            ))
            .exec(&mut *db)
            .await?
        } else {
            models::Article::filter(
                models::Article::fields()
                    .status()
                    .eq(ArticleStatus::Published.as_str()),
            )
            .exec(&mut *db)
            .await?
        };
        let mut items = rows
            .into_iter()
            .map(article_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        sort_articles_public(&mut items);
        Ok(items.into_iter().map(|item| item.as_card()).collect())
    }

    /// Full published article for the public detail view.
    pub async fn get_published_article(&self, slug: &str) -> Result<Article, AppError> {
        let mut db = self.inner.lock().await;
        let row = models::Article::filter_by_slug(slug)
            .first()
            .exec(&mut *db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("article '{slug}' not found")))?;
        let article = article_from_row(row)?;
        if article.status != ArticleStatus::Published {
            return Err(AppError::NotFound(format!("article '{slug}' not found")));
        }
        Ok(article)
    }

    /// All articles for the admin dashboard, cards only.
    pub async fn admin_list_articles(&self) -> Result<Vec<Article>, AppError> {
        let mut db = self.inner.lock().await;
        let rows = models::Article::all().exec(&mut *db).await?;
        let mut items = rows
            .into_iter()
            .map(article_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        items.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then(a.slug.cmp(&b.slug)));
        Ok(items.into_iter().map(|item| item.as_card()).collect())
    }

    /// Full article (including document and compiled artifact) for the editor.
    pub async fn admin_get_article(&self, slug: &str) -> Result<Article, AppError> {
        let mut db = self.inner.lock().await;
        let row = models::Article::filter_by_slug(slug)
            .first()
            .exec(&mut *db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("article '{slug}' not found")))?;
        article_from_row(row)
    }

    pub async fn article_exists(&self, slug: &str) -> Result<bool, AppError> {
        let mut db = self.inner.lock().await;
        Ok(models::Article::filter_by_slug(slug)
            .first()
            .exec(&mut *db)
            .await?
            .is_some())
    }

    pub async fn insert_article(&self, fields: ArticleFields) -> Result<Article, AppError> {
        let mut db = self.inner.lock().await;
        let row = toasty::create!(models::Article {
            slug: fields.slug,
            title: fields.title,
            category: fields.category,
            summary: fields.summary,
            status: fields.status,
            featured: fields.featured,
            sort_order: fields.sort_order,
            published_at: fields.published_at,
            reading_time_minutes: fields.reading_time_minutes,
            sections: fields.sections,
            document: fields.document,
            document_version: fields.document_version,
            compiled: fields.compiled,
            content_hash: fields.content_hash,
        })
        .exec(&mut *db)
        .await?;
        article_from_row(row)
    }

    /// Saves every writable field of an article identified by its (current)
    /// slug.
    pub async fn save_article(&self, fields: &ArticleFields) -> Result<Article, AppError> {
        let mut db = self.inner.lock().await;
        let slug = fields.slug.clone();
        let title = fields.title.clone();
        let category = fields.category.clone();
        let summary = fields.summary.clone();
        let status = fields.status.clone();
        let featured = fields.featured;
        let sort_order = fields.sort_order;
        let published_at = fields.published_at.clone();
        let reading_time_minutes = fields.reading_time_minutes;
        let sections = fields.sections.clone();
        let document = fields.document.clone();
        let document_version = fields.document_version;
        let compiled = fields.compiled.clone();
        let content_hash = fields.content_hash.clone();
        toasty::update!(models::Article::filter_by_slug(slug) {
            title,
            category,
            summary,
            status,
            featured,
            sort_order,
            published_at,
            reading_time_minutes,
            sections,
            document,
            document_version,
            compiled,
            content_hash,
        })
        .exec(&mut *db)
        .await?;
        let slug = fields.slug.clone();
        let row = models::Article::filter_by_slug(slug)
            .first()
            .exec(&mut *db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("article '{}' not found", fields.slug)))?;
        article_from_row(row)
    }

    pub async fn delete_article(&self, slug: &str) -> Result<(), AppError> {
        let mut db = self.inner.lock().await;
        models::Article::filter_by_slug(slug)
            .delete()
            .exec(&mut *db)
            .await?;
        Ok(())
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

    // Comments

    pub async fn insert_comment(&self, input: NewComment<'_>) -> Result<CommentInfo, AppError> {
        let NewComment {
            article_slug,
            identity_id,
            author_name,
            body,
            status,
            ip_hash,
            parent_id,
            thread_id,
        } = input;
        let mut db = self.inner.lock().await;
        let created_at = now_rfc3339();
        let row = toasty::create!(models::Comment {
            parent_id: parent_id.map(str::to_owned),
            thread_id: thread_id.map(str::to_owned),
            article_slug: article_slug.to_owned(),
            identity_id: identity_id.map(str::to_owned),
            author_name: author_name.to_owned(),
            body: body.to_owned(),
            status: status.to_owned(),
            ip_hash: ip_hash.to_owned(),
            created_at,
        })
        .exec(&mut *db)
        .await?;
        Ok(comment_from_row(row))
    }

    /// Visible discussion entries, including removed ancestors of approved replies.
    pub async fn public_discussion(
        &self,
        article_slug: &str,
    ) -> Result<Vec<CommentInfo>, AppError> {
        let mut db = self.inner.lock().await;
        let rows =
            models::Comment::filter(models::Comment::fields().article_slug().eq(article_slug))
                .exec(&mut *db)
                .await?;
        let items: Vec<CommentInfo> = rows.into_iter().map(comment_from_row).collect();
        let by_id: std::collections::HashMap<_, _> =
            items.iter().map(|c| (c.id.as_str(), c)).collect();
        let mut visible = std::collections::HashSet::new();
        for comment in items.iter().filter(|c| c.status == "approved") {
            let mut current = Some(comment);
            while let Some(node) = current {
                if !matches!(node.status.as_str(), "approved" | "removed") {
                    break;
                }
                if !visible.insert(node.id.clone()) {
                    break;
                }
                current = node
                    .parent_id
                    .as_deref()
                    .and_then(|id| by_id.get(id).copied());
            }
        }
        let mut items: Vec<_> = items
            .into_iter()
            .filter(|c| visible.contains(&c.id))
            .collect();
        items.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        Ok(items)
    }

    /// All comments matching `status` (None = every status), newest first.
    pub async fn list_admin_comments(
        &self,
        status: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<CommentInfo>, i64), AppError> {
        let mut db = self.inner.lock().await;
        let rows = match status {
            Some(status) => {
                models::Comment::filter(models::Comment::fields().status().eq(status))
                    .exec(&mut *db)
                    .await?
            }
            None => models::Comment::all().exec(&mut *db).await?,
        };
        let mut items: Vec<CommentInfo> = rows.into_iter().map(comment_from_row).collect();
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let total = items.len() as i64;
        let start = page.saturating_sub(1).saturating_mul(per_page).max(0) as usize;
        let page_items = items
            .into_iter()
            .skip(start)
            .take(per_page.max(0) as usize)
            .collect();
        Ok((page_items, total))
    }

    pub async fn get_comment(&self, id: &str) -> Result<Option<CommentInfo>, AppError> {
        let mut db = self.inner.lock().await;
        let Ok(uuid) = uuid::Uuid::parse_str(id) else {
            return Ok(None);
        };
        let row = models::Comment::filter_by_id(uuid)
            .first()
            .exec(&mut *db)
            .await?;
        Ok(row.map(comment_from_row))
    }

    pub async fn approve_comment(&self, id: &str) -> Result<(), AppError> {
        let mut db = self.inner.lock().await;
        let uuid = parse_uuid(id)?;
        toasty::update!(models::Comment::filter_by_id(uuid) {
            status: "approved".to_owned(),
        })
        .exec(&mut *db)
        .await?;
        Ok(())
    }

    pub async fn delete_comment(&self, id: &str) -> Result<(), AppError> {
        let mut db = self.inner.lock().await;
        let uuid = parse_uuid(id)?;
        toasty::update!(models::Comment::filter_by_id(uuid) {
            status: "removed".to_owned(),
            body: String::new(),
            author_name: "Removed comment".to_owned(),
            identity_id: None::<String>,
        })
        .exec(&mut *db)
        .await?;
        Ok(())
    }

    pub async fn count_comments_by_ip_since(
        &self,
        ip_hash: &str,
        since: &str,
        article_slug: Option<&str>,
    ) -> Result<i64, AppError> {
        let mut db = self.inner.lock().await;
        let rows = models::Comment::filter(models::Comment::fields().ip_hash().eq(ip_hash))
            .exec(&mut *db)
            .await?;
        let count = rows
            .into_iter()
            .map(comment_from_row)
            .filter(|comment| comment.created_at.as_str() >= since)
            .filter(|comment| article_slug.is_none_or(|slug| comment.article_slug == slug))
            .count() as i64;
        Ok(count)
    }

    // Identities

    pub async fn find_identity(
        &self,
        provider: &str,
        subject: &str,
    ) -> Result<Option<IdentityInfo>, AppError> {
        let mut db = self.inner.lock().await;
        let rows = models::Identity::filter(toasty::stmt::Expr::and(
            models::Identity::fields().provider().eq(provider),
            models::Identity::fields().subject().eq(subject),
        ))
        .exec(&mut *db)
        .await?;
        Ok(rows.into_iter().next().map(identity_from_row))
    }

    pub async fn upsert_identity(
        &self,
        provider: &str,
        subject: &str,
        email: &str,
        name: &str,
        avatar_url: Option<&str>,
    ) -> Result<IdentityInfo, AppError> {
        let mut db = self.inner.lock().await;
        let existing = self.find_identity_inner(&mut db, provider, subject).await?;
        if let Some(row) = existing {
            let id = row.id;
            toasty::update!(models::Identity::filter_by_id(id) {
                email: email.to_owned(),
                name: name.to_owned(),
                avatar_url: avatar_url.map(str::to_owned),
            })
            .exec(&mut *db)
            .await?;
            let row = models::Identity::filter_by_id(id)
                .first()
                .exec(&mut *db)
                .await?
                .ok_or_else(|| AppError::Internal("identity vanished during update".to_owned()))?;
            return Ok(identity_from_row(row));
        }
        let row = toasty::create!(models::Identity {
            provider: provider.to_owned(),
            subject: subject.to_owned(),
            email: email.to_owned(),
            name: name.to_owned(),
            avatar_url: avatar_url.map(str::to_owned),
        })
        .exec(&mut *db)
        .await?;
        Ok(identity_from_row(row))
    }

    async fn find_identity_inner(
        &self,
        db: &mut toasty::Db,
        provider: &str,
        subject: &str,
    ) -> Result<Option<models::Identity>, AppError> {
        let rows = models::Identity::filter(toasty::stmt::Expr::and(
            models::Identity::fields().provider().eq(provider),
            models::Identity::fields().subject().eq(subject),
        ))
        .exec(db)
        .await?;
        Ok(rows.into_iter().next())
    }

    /// Resolves a commenter identity by its stored id string.
    pub async fn get_identity_by_id(
        &self,
        id: &str,
    ) -> Result<Option<crate::domain::CommenterIdentity>, AppError> {
        let mut db = self.inner.lock().await;
        let Ok(uuid) = uuid::Uuid::parse_str(id) else {
            return Ok(None);
        };
        let row = models::Identity::filter_by_id(uuid)
            .first()
            .exec(&mut *db)
            .await?;
        Ok(row.map(|row| crate::domain::CommenterIdentity {
            id: row.id.to_string(),
            provider: row.provider,
            name: row.name,
            email: row.email,
            avatar_url: row.avatar_url,
        }))
    }

    /// Resolve the provider's immutable subject for admin authorization.
    pub async fn identity_subject(&self, id: &str) -> Result<Option<(String, String)>, AppError> {
        let mut db = self.inner.lock().await;
        let Ok(uuid) = uuid::Uuid::parse_str(id) else {
            return Ok(None);
        };
        let row = models::Identity::filter_by_id(uuid)
            .first()
            .exec(&mut *db)
            .await?;
        Ok(row.map(|row| (row.provider, row.subject)))
    }

    // Sessions

    pub async fn create_session(
        &self,
        kind: &str,
        token_hash: &str,
        identity_id: Option<&str>,
        ttl_hours: i64,
    ) -> Result<(), AppError> {
        let mut db = self.inner.lock().await;
        self.delete_expired_sessions_inner(&mut db).await?;
        toasty::create!(models::Session {
            token_hash: token_hash.to_owned(),
            kind: kind.to_owned(),
            identity_id: identity_id.map(str::to_owned),
            created_at: now_rfc3339(),
            expires_at: crate::util::rfc3339_plus_hours(ttl_hours),
        })
        .exec(&mut *db)
        .await?;
        Ok(())
    }

    /// Returns the live session for a token hash, or None when missing or
    /// expired. Expired sessions are deleted opportunistically.
    pub async fn get_live_session(
        &self,
        token_hash: &str,
    ) -> Result<Option<SessionInfo>, AppError> {
        let mut db = self.inner.lock().await;
        let row = models::Session::filter_by_token_hash(token_hash)
            .first()
            .exec(&mut *db)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let now = now_rfc3339();
        if row.expires_at < now {
            models::Session::filter_by_id(row.id)
                .delete()
                .exec(&mut *db)
                .await?;
            return Ok(None);
        }
        Ok(Some(SessionInfo {
            kind: row.kind,
            identity_id: row.identity_id,
            expires_at: row.expires_at,
        }))
    }

    pub async fn delete_session(&self, token_hash: &str) -> Result<(), AppError> {
        let mut db = self.inner.lock().await;
        models::Session::filter_by_token_hash(token_hash)
            .delete()
            .exec(&mut *db)
            .await?;
        Ok(())
    }

    async fn delete_expired_sessions_inner(&self, db: &mut toasty::Db) -> Result<(), AppError> {
        let now = now_rfc3339();
        let rows = models::Session::filter(models::Session::fields().expires_at().lt(now))
            .exec(db)
            .await?;
        for row in rows {
            models::Session::filter_by_id(row.id)
                .delete()
                .exec(db)
                .await?;
        }
        Ok(())
    }
}

/// Content hash backing public ETags: a digest over every reader-visible
/// field of an article.
pub fn compute_content_hash(fields: &ArticleFields) -> String {
    let mut input = String::new();
    input.push_str(fields.slug.as_str());
    input.push('\u{1f}');
    input.push_str(fields.title.as_str());
    input.push('\u{1f}');
    input.push_str(fields.category.as_str());
    input.push('\u{1f}');
    input.push_str(fields.summary.as_str());
    input.push('\u{1f}');
    input.push_str(if fields.featured { "featured" } else { "plain" });
    input.push('\u{1f}');
    input.push_str(&fields.published_at.clone().unwrap_or_default());
    input.push('\u{1f}');
    input.push_str(&fields.reading_time_minutes.unwrap_or(0).to_string());
    input.push('\u{1f}');
    input.push_str(&fields.sections);
    input.push('\u{1f}');
    input.push_str(&fields.document);
    input.push('\u{1f}');
    input.push_str(&fields.compiled);
    sha256_hex(input.as_bytes())
}

fn sort_articles_public(articles: &mut [Article]) {
    articles.sort_by(|a, b| {
        let a_key = a.published_at.clone().unwrap_or_default();
        let b_key = b.published_at.clone().unwrap_or_default();
        b_key
            .cmp(&a_key)
            .then(a.sort_order.cmp(&b.sort_order))
            .then(a.slug.cmp(&b.slug))
    });
}

fn parse_uuid(value: &str) -> Result<uuid::Uuid, AppError> {
    uuid::Uuid::parse_str(value)
        .map_err(|_| AppError::NotFound(format!("no record with id '{value}'")))
}

async fn apply_schema(db: &mut toasty::Db) -> Result<(), AppError> {
    for statement in SCHEMA_STATEMENTS.iter().copied() {
        toasty::sql::statement(statement).exec(&mut *db).await?;
    }

    // SQLite cannot add a column idempotently; tolerate duplicate-column
    // errors from databases that already carry the new articles columns.
    for (column, statement) in ARTICLE_COLUMN_ADDITIONS
        .iter()
        .chain(COMMENT_COLUMN_ADDITIONS)
        .copied()
    {
        if let Err(err) = toasty::sql::statement(statement).exec(&mut *db).await {
            let message = err.to_string();
            if !message.contains("duplicate column name") {
                return Err(AppError::Database(message));
            }
            tracing::debug!(column, "column already present");
        }
    }

    Ok(())
}

/// Idempotent DDL matching the toasty-generated storage layout. Fresh
/// databases get the full schema; existing ones only gain missing tables and
/// indexes.
const SCHEMA_STATEMENTS: &[&str] = &[
    r#"CREATE TABLE IF NOT EXISTS "projects" ("id" BLOB NOT NULL, "slug" TEXT NOT NULL, "title" TEXT NOT NULL, "category" TEXT NOT NULL, "summary" TEXT NOT NULL, "tags" TEXT NOT NULL, "sections" TEXT NOT NULL, "hero" TEXT, "has_detail_page" BOOLEAN NOT NULL, "featured" BOOLEAN NOT NULL, "sort_order" BIGINT NOT NULL, "repo_url" TEXT, "related_slugs" TEXT NOT NULL, PRIMARY KEY ("id"))"#,
    r#"CREATE UNIQUE INDEX IF NOT EXISTS "index_projects_by_slug" ON "projects" ("slug")"#,
    // articles: base columns shared with pre-document databases
    r#"CREATE TABLE IF NOT EXISTS "articles" ("id" BLOB NOT NULL, "slug" TEXT NOT NULL, "title" TEXT NOT NULL, "category" TEXT NOT NULL, "summary" TEXT NOT NULL, "status" TEXT NOT NULL, "featured" BOOLEAN NOT NULL, "sort_order" BIGINT NOT NULL, "published_at" TEXT, "reading_time_minutes" BIGINT, "sections" TEXT NOT NULL, "document" TEXT NOT NULL DEFAULT '', "document_version" BIGINT NOT NULL DEFAULT 0, "compiled" TEXT NOT NULL DEFAULT '', "content_hash" TEXT NOT NULL DEFAULT '', PRIMARY KEY ("id"))"#,
    r#"CREATE UNIQUE INDEX IF NOT EXISTS "index_articles_by_slug" ON "articles" ("slug")"#,
    r#"CREATE TABLE IF NOT EXISTS "contact_messages" ("id" BLOB NOT NULL, "name" TEXT NOT NULL, "email" TEXT NOT NULL, "message" TEXT NOT NULL, PRIMARY KEY ("id"))"#,
    r#"CREATE TABLE IF NOT EXISTS "newsletter_subscribers" ("id" BLOB NOT NULL, "email" TEXT NOT NULL, PRIMARY KEY ("id"))"#,
    r#"CREATE UNIQUE INDEX IF NOT EXISTS "index_newsletter_subscribers_by_email" ON "newsletter_subscribers" ("email")"#,
    r#"CREATE TABLE IF NOT EXISTS "comments" ("id" BLOB NOT NULL, "article_slug" TEXT NOT NULL, "identity_id" TEXT, "author_name" TEXT NOT NULL, "body" TEXT NOT NULL, "status" TEXT NOT NULL, "ip_hash" TEXT NOT NULL, "created_at" TEXT NOT NULL, PRIMARY KEY ("id"))"#,
    r#"CREATE INDEX IF NOT EXISTS "index_comments_by_article_slug" ON "comments" ("article_slug")"#,
    r#"CREATE INDEX IF NOT EXISTS "index_comments_by_status" ON "comments" ("status")"#,
    r#"CREATE INDEX IF NOT EXISTS "index_comments_by_ip_hash" ON "comments" ("ip_hash")"#,
    r#"CREATE TABLE IF NOT EXISTS "identities" ("id" BLOB NOT NULL, "provider" TEXT NOT NULL, "subject" TEXT NOT NULL, "email" TEXT NOT NULL, "name" TEXT NOT NULL, "avatar_url" TEXT, PRIMARY KEY ("id"))"#,
    r#"CREATE INDEX IF NOT EXISTS "index_identities_by_provider" ON "identities" ("provider")"#,
    r#"CREATE TABLE IF NOT EXISTS "sessions" ("id" BLOB NOT NULL, "token_hash" TEXT NOT NULL, "kind" TEXT NOT NULL, "identity_id" TEXT, "created_at" TEXT NOT NULL, "expires_at" TEXT NOT NULL, PRIMARY KEY ("id"))"#,
    r#"CREATE UNIQUE INDEX IF NOT EXISTS "index_sessions_by_token_hash" ON "sessions" ("token_hash")"#,
    r#"CREATE INDEX IF NOT EXISTS "index_sessions_by_expires_at" ON "sessions" ("expires_at")"#,
];

/// Column additions for databases created before articles stored Publisle
/// documents.
const COMMENT_COLUMN_ADDITIONS: &[(&str, &str)] = &[
    (
        "parent_id",
        r#"ALTER TABLE "comments" ADD COLUMN "parent_id" TEXT"#,
    ),
    (
        "thread_id",
        r#"ALTER TABLE "comments" ADD COLUMN "thread_id" TEXT"#,
    ),
];

const ARTICLE_COLUMN_ADDITIONS: &[(&str, &str)] = &[
    (
        "document",
        r#"ALTER TABLE "articles" ADD COLUMN "document" TEXT NOT NULL DEFAULT ''"#,
    ),
    (
        "document_version",
        r#"ALTER TABLE "articles" ADD COLUMN "document_version" BIGINT NOT NULL DEFAULT 0"#,
    ),
    (
        "compiled",
        r#"ALTER TABLE "articles" ADD COLUMN "compiled" TEXT NOT NULL DEFAULT ''"#,
    ),
    (
        "content_hash",
        r#"ALTER TABLE "articles" ADD COLUMN "content_hash" TEXT NOT NULL DEFAULT ''"#,
    ),
];

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
        crate::db::models::NewsletterSubscriber,
        crate::db::models::Comment,
        crate::db::models::Identity,
        crate::db::models::Session
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
    let document = if row.document.is_empty() {
        None
    } else {
        Some(serde_json::from_str(&row.document)?)
    };
    let compiled = if row.compiled.is_empty() {
        None
    } else {
        Some(serde_json::from_str(&row.compiled)?)
    };
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
        document,
        compiled,
        content_hash: row.content_hash,
    })
}

fn comment_from_row(row: models::Comment) -> CommentInfo {
    CommentInfo {
        parent_id: row.parent_id,
        thread_id: row.thread_id,
        id: row.id.to_string(),
        article_slug: row.article_slug,
        identity_id: row.identity_id,
        author_name: row.author_name,
        body: row.body,
        status: row.status,
        created_at: row.created_at,
        email: None,
    }
}

fn identity_from_row(row: models::Identity) -> IdentityInfo {
    IdentityInfo {
        id: row.id.to_string(),
        provider: row.provider,
        subject: row.subject,
        email: row.email,
        name: row.name,
        avatar_url: row.avatar_url,
    }
}
