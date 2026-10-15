use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Serialize;

use crate::auth::extract::AdminSession;
use crate::db::{ArticleFields, Db, compute_content_hash};
use crate::domain::document;
use crate::domain::{Article, ArticleMutationResponse, ArticleWriteRequest};
use crate::error::AppError;
use crate::util::now_rfc3339;

const MAX_TITLE_LEN: usize = 200;
const MAX_CATEGORY_LEN: usize = 80;
const MAX_SUMMARY_LEN: usize = 500;
const MAX_SLUG_LEN: usize = 120;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct AdminArticleListResponse {
    pub items: Vec<Article>,
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/admin/articles",
    tag = "admin",
    responses(
        (status = 200, description = "All articles, cards only", body = AdminArticleListResponse),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[get("/articles")]
pub async fn list_articles(
    db: web::Data<Db>,
    session: AdminSession,
) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;
    let items = db.admin_list_articles().await?;
    Ok(HttpResponse::Ok().json(AdminArticleListResponse { items }))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/api/v1/admin/articles",
    tag = "admin",
    request_body = ArticleWriteRequest,
    responses(
        (status = 201, description = "Article created as a draft", body = crate::domain::Article),
        (status = 409, description = "Slug already exists", body = crate::error::ErrorBody),
        (status = 422, description = "Invalid payload", body = crate::error::ErrorBody),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[post("/articles")]
pub async fn create_article(
    db: web::Data<Db>,
    payload: web::Json<ArticleWriteRequest>,
    session: AdminSession,
) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;
    create(&db, payload.into_inner()).await
}

pub(crate) async fn create(db: &Db, input: ArticleWriteRequest) -> Result<HttpResponse, AppError> {
    let title = required(&input.title, "title", MAX_TITLE_LEN)?;
    let category = required(&input.category, "category", MAX_CATEGORY_LEN)?;
    let summary = required(&input.summary, "summary", MAX_SUMMARY_LEN)?;
    let slug = match input
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(slug) => slug.to_owned(),
        None => slugify(&title),
    };
    validate_slug(&slug)?;
    if db.article_exists(&slug).await? {
        return Err(AppError::Conflict(format!("slug '{slug}' already exists")));
    }

    let (doc_json, doc_version) =
        prepare_document(input.document.as_ref(), input.document_version)?;
    let compiled_json = prepare_compiled(input.compiled.as_ref())?;

    let fields = ArticleFields {
        revision: 1,
        slug,
        title,
        category,
        summary,
        status: crate::domain::ArticleStatus::Draft.as_str().to_owned(),
        featured: input.featured.unwrap_or(false),
        sort_order: input.sort_order.unwrap_or(0),
        published_at: None,
        reading_time_minutes: input.reading_time_minutes,
        sections: "[]".to_owned(),
        document: doc_json,
        document_version: doc_version,
        compiled: compiled_json,
        content_hash: String::new(),
    };
    let fields = ArticleFields {
        content_hash: compute_content_hash(&fields),
        ..fields
    };

    let article = db.insert_article(fields).await?;
    Ok(HttpResponse::Created().json(article))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/admin/articles/{slug}",
    tag = "admin",
    params(("slug" = String, Path, description = "Article slug")),
    responses(
        (status = 200, description = "Full article", body = crate::domain::Article),
        (status = 404, description = "Article not found", body = crate::error::ErrorBody),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[get("/articles/{slug}")]
pub async fn get_article(
    db: web::Data<Db>,
    path: web::Path<String>,
    session: AdminSession,
) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;
    let article = db.admin_get_article(&path).await?;
    Ok(HttpResponse::Ok().json(article))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    put,
    path = "/api/v1/admin/articles/{slug}",
    tag = "admin",
    request_body = ArticleWriteRequest,
    params(("slug" = String, Path, description = "Article slug")),
    responses(
        (status = 200, description = "Article updated", body = crate::domain::Article),
        (status = 404, description = "Article not found", body = crate::error::ErrorBody),
        (status = 422, description = "Invalid payload", body = crate::error::ErrorBody),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[put("/articles/{slug}")]
pub async fn update_article(
    db: web::Data<Db>,
    path: web::Path<String>,
    payload: web::Json<ArticleWriteRequest>,
    session: AdminSession,
) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;
    update(&db, path.into_inner(), payload.into_inner(), false).await
}

pub(crate) async fn update(
    db: &Db,
    slug: String,
    input: ArticleWriteRequest,
    draft_only: bool,
) -> Result<HttpResponse, AppError> {
    if let Some(requested) = input
        .slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        && requested != slug
    {
        return Err(AppError::Unprocessable(
            "slugs are immutable; create a new article to rename".to_owned(),
        ));
    }

    let mut current_fields = article_fields(db.admin_get_article(&slug).await?)?;
    check_revision(input.expected_revision, current_fields.revision)?;
    if draft_only && current_fields.status != "draft" {
        return Err(AppError::Forbidden("MCP can only edit drafts".into()));
    }

    if let Some(title) = &input.title {
        current_fields.title = bounded(title, "title", MAX_TITLE_LEN)?;
    }
    if let Some(category) = &input.category {
        current_fields.category = bounded(category, "category", MAX_CATEGORY_LEN)?;
    }
    if let Some(summary) = &input.summary {
        current_fields.summary = bounded(summary, "summary", MAX_SUMMARY_LEN)?;
    }
    if let Some(featured) = input.featured {
        current_fields.featured = featured;
    }
    if let Some(sort_order) = input.sort_order {
        current_fields.sort_order = sort_order;
    }
    if let Some(reading_time) = input.reading_time_minutes {
        current_fields.reading_time_minutes = Some(reading_time);
    }
    if input.document.is_some() {
        let (doc_json, doc_version) =
            prepare_document(input.document.as_ref(), input.document_version)?;
        current_fields.compiled.clear();
        current_fields.document = doc_json;
        current_fields.document_version = doc_version;
    } else if input.document_version.is_some() {
        return Err(AppError::Unprocessable(
            "document_version requires a document".to_owned(),
        ));
    }
    if current_fields.document.is_empty() && !current_fields.sections.is_empty() {
        let legacy: Vec<crate::domain::ContentSection> =
            serde_json::from_str(&current_fields.sections)?;
        current_fields.document =
            serde_json::to_string(&document::from_sections(&current_fields.title, &legacy))?;
        current_fields.document_version = document::DOCUMENT_SCHEMA_VERSION;
    }
    if input.compiled.is_some() {
        current_fields.compiled = prepare_compiled(input.compiled.as_ref())?;
    }

    if current_fields.status == "published" && current_fields.compiled.is_empty() {
        return Err(AppError::Unprocessable(
            "published document updates require a compiled artifact; unpublish to save a draft"
                .to_owned(),
        ));
    }
    current_fields.content_hash = compute_content_hash(&current_fields);
    let article = db.save_article(&current_fields, draft_only).await?;
    Ok(HttpResponse::Ok().json(article))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/api/v1/admin/articles/{slug}/publish",
    tag = "admin",
    params(("slug" = String, Path, description = "Article slug")),
    responses(
        (status = 200, description = "Article published", body = ArticleMutationResponse),
        (status = 404, description = "Article not found", body = crate::error::ErrorBody),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[post("/articles/{slug}/publish")]
pub async fn publish_article(
    db: web::Data<Db>,
    path: web::Path<String>,
    session: AdminSession,
    payload: web::Json<ArticleWriteRequest>,
) -> Result<HttpResponse, AppError> {
    set_status(
        db,
        path,
        session,
        payload.expected_revision,
        crate::domain::ArticleStatus::Published,
    )
    .await
}

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/api/v1/admin/articles/{slug}/unpublish",
    tag = "admin",
    params(("slug" = String, Path, description = "Article slug")),
    responses(
        (status = 200, description = "Article returned to draft", body = ArticleMutationResponse),
        (status = 404, description = "Article not found", body = crate::error::ErrorBody),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[post("/articles/{slug}/unpublish")]
pub async fn unpublish_article(
    db: web::Data<Db>,
    path: web::Path<String>,
    session: AdminSession,
    payload: web::Json<ArticleWriteRequest>,
) -> Result<HttpResponse, AppError> {
    set_status(
        db,
        path,
        session,
        payload.expected_revision,
        crate::domain::ArticleStatus::Draft,
    )
    .await
}

#[cfg_attr(feature = "swagger", utoipa::path(
    delete,
    path = "/api/v1/admin/articles/{slug}",
    tag = "admin",
    params(("slug" = String, Path, description = "Article slug")),
    responses(
        (status = 200, description = "Article deleted"),
        (status = 404, description = "Article not found", body = crate::error::ErrorBody),
        (status = 401, description = "Not signed in", body = crate::error::ErrorBody)
    )
))]
#[delete("/articles/{slug}")]
pub async fn delete_article(
    db: web::Data<Db>,
    path: web::Path<String>,
    session: AdminSession,
    payload: web::Json<ArticleWriteRequest>,
) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;
    // Ensure it exists first so the response is 404, not a silent delete.
    let slug = path.into_inner();
    db.admin_get_article(&slug).await?;
    let expected = payload.expected_revision.ok_or_else(|| {
        AppError::Conflict("expected_revision is required; reload the article".into())
    })?;
    db.delete_article(&slug, expected).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({ "deleted": slug })))
}

async fn set_status(
    db: web::Data<Db>,
    path: web::Path<String>,
    session: AdminSession,
    expected_revision: Option<i64>,
    status: crate::domain::ArticleStatus,
) -> Result<HttpResponse, AppError> {
    session.validate(&db).await?;
    let slug = path.into_inner();
    let mut fields = article_fields(db.admin_get_article(&slug).await?)?;
    check_revision(expected_revision, fields.revision)?;
    if status == crate::domain::ArticleStatus::Published {
        document::validate_document(&fields.document)?;
        document::validate_compiled(&fields.compiled)?;
    }
    fields.status = status.as_str().to_owned();
    if status == crate::domain::ArticleStatus::Published && fields.published_at.is_none() {
        fields.published_at = Some(now_rfc3339());
    }
    fields.content_hash = compute_content_hash(&fields);
    let saved = db.save_article(&fields, false).await?;
    Ok(HttpResponse::Ok().json(ArticleMutationResponse {
        revision: saved.revision,
        slug: fields.slug,
        status: fields.status,
        content_hash: fields.content_hash,
    }))
}

fn article_fields(article: Article) -> Result<ArticleFields, AppError> {
    let document = match &article.document {
        Some(value) => serde_json::to_string(value)
            .map_err(|_| AppError::Internal("stored document is invalid".to_owned()))?,
        None => String::new(),
    };
    let compiled = match &article.compiled {
        Some(value) => serde_json::to_string(value)
            .map_err(|_| AppError::Internal("stored compiled is invalid".to_owned()))?,
        None => String::new(),
    };
    Ok(ArticleFields {
        revision: article.revision,
        slug: article.slug,
        title: article.title,
        category: article.category,
        summary: article.summary,
        status: article.status.as_str().to_owned(),
        featured: article.featured,
        sort_order: article.sort_order,
        published_at: article.published_at,
        reading_time_minutes: article.reading_time_minutes,
        sections: serde_json::to_string(&article.sections).unwrap_or_else(|_| "[]".to_owned()),
        document,
        document_version: if article.document.is_some() {
            document::DOCUMENT_SCHEMA_VERSION
        } else {
            0
        },
        compiled,
        content_hash: article.content_hash,
    })
}

fn prepare_document(
    document: Option<&serde_json::Value>,
    requested_version: Option<i64>,
) -> Result<(String, i64), AppError> {
    match document {
        None if requested_version.is_some() => Err(AppError::Unprocessable(
            "document_version requires a document".to_owned(),
        )),
        None => Ok((String::new(), 0)),
        Some(value) => {
            let raw = serde_json::to_string(value).map_err(|err| {
                AppError::Unprocessable(format!("document is not valid JSON: {err}"))
            })?;
            let version = document::validate_document(&raw)?;
            if let Some(requested) = requested_version
                && requested != version
            {
                return Err(AppError::Unprocessable(format!(
                    "document_version {requested} does not match the document schemaVersion {version}"
                )));
            }
            Ok((raw, version))
        }
    }
}

fn prepare_compiled(compiled: Option<&serde_json::Value>) -> Result<String, AppError> {
    match compiled {
        None => Ok(String::new()),
        Some(value) => {
            let raw = serde_json::to_string(value).map_err(|err| {
                AppError::Unprocessable(format!("compiled artifact is not valid JSON: {err}"))
            })?;
            document::validate_compiled(&raw)?;
            Ok(raw)
        }
    }
}

fn required(value: &Option<String>, name: &str, max: usize) -> Result<String, AppError> {
    let trimmed = value
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Unprocessable(format!("{name} is required")))?;
    bounded(trimmed, name, max)
}

fn bounded(value: &str, name: &str, max: usize) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max {
        return Err(AppError::Unprocessable(format!(
            "{name} must be between 1 and {max} characters"
        )));
    }
    Ok(trimmed.to_owned())
}

fn validate_slug(slug: &str) -> Result<(), AppError> {
    let valid = !slug.is_empty()
        && slug.len() <= MAX_SLUG_LEN
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
        && !slug.contains("--");
    if valid {
        Ok(())
    } else {
        Err(AppError::Unprocessable(
            "slug must be lowercase letters, digits, and single hyphens (no leading or trailing hyphen)".to_owned(),
        ))
    }
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut last_hyphen = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            last_hyphen = false;
        } else if !last_hyphen && !slug.is_empty() {
            slug.push('-');
            last_hyphen = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn check_revision(expected: Option<i64>, current: i64) -> Result<(), AppError> {
    if expected != Some(current) {
        return Err(AppError::Conflict(
            "article changed or expected_revision missing; reload before saving".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_titles() {
        assert_eq!(
            slugify("How to Validate a Semantic Color System"),
            "how-to-validate-a-semantic-color-system"
        );
        assert_eq!(
            slugify("  Spaces, Commas;  and More!  "),
            "spaces-commas-and-more"
        );
        assert_eq!(slugify("---"), "");
    }

    #[test]
    fn validates_slugs() {
        assert!(validate_slug("valid-slug-1").is_ok());
        assert!(validate_slug("UPPER").is_err());
        assert!(validate_slug("-leading").is_err());
        assert!(validate_slug("double--hyphen").is_err());
        assert!(validate_slug("").is_err());
    }
}
