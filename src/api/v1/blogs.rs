use actix_web::{HttpRequest, HttpResponse, get, http::header, web};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::db::Db;
use crate::domain::ArticleListResponse;
use crate::error::AppError;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::IntoParams))]
pub struct BlogQuery {
    pub featured: Option<bool>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

const LIST_CACHE_CONTROL: &str = "public, max-age=60, stale-while-revalidate=300";
const DETAIL_CACHE_CONTROL: &str = "public, max-age=300, stale-while-revalidate=600";

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/blogs",
    tag = "blogs",
    params(BlogQuery),
    responses(
        (status = 200, description = "Published blog list", body = ArticleListResponse),
        (status = 304, description = "Not modified")
    )
))]
#[get("/blogs")]
pub async fn list_blogs(
    db: web::Data<Db>,
    query: web::Query<BlogQuery>,
    request: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let items = db.list_published_articles(query.featured).await?;
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
    let total = items.len() as i64;
    let start = page.saturating_sub(1).saturating_mul(per_page) as usize;
    let items = items
        .into_iter()
        .skip(start)
        .take(per_page as usize)
        .collect();
    let body = serde_json::to_string(&ArticleListResponse {
        items,
        page,
        per_page,
        total,
    })?;
    Ok(cached_response(&request, body, LIST_CACHE_CONTROL, None))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/blogs/{slug}",
    tag = "blogs",
    params(("slug" = String, Path, description = "Blog slug")),
    responses(
        (status = 200, description = "Published blog detail", body = crate::domain::Article),
        (status = 304, description = "Not modified"),
        (status = 404, description = "Blog not found", body = crate::error::ErrorBody)
    )
))]
#[get("/blogs/{slug}")]
pub async fn get_blog(
    db: web::Data<Db>,
    path: web::Path<String>,
    request: HttpRequest,
) -> Result<HttpResponse, AppError> {
    let mut article = db.get_published_article(&path).await?;
    article.document = None;
    let etag = (!article.content_hash.is_empty()).then(|| article.content_hash.clone());
    let body = serde_json::to_string(&article)?;
    Ok(cached_response(&request, body, DETAIL_CACHE_CONTROL, etag))
}

/// Builds a JSON response with ETag/304 handling and shared caching headers.
pub fn cached_response(
    request: &HttpRequest,
    body: String,
    cache_control: &str,
    etag: Option<String>,
) -> HttpResponse {
    let etag = etag.unwrap_or_else(|| {
        let mut hasher = Sha256::new();
        hasher.update(body.as_bytes());
        hex::encode(hasher.finalize())
    });
    let quoted = format!("\"{etag}\"");

    if request_if_none_match(request)
        .iter()
        .any(|candidate| candidate == &etag || candidate == "*")
    {
        return HttpResponse::NotModified()
            .insert_header((header::ETAG, quoted.as_str()))
            .insert_header((header::CACHE_CONTROL, cache_control))
            .finish();
    }

    HttpResponse::Ok()
        .content_type("application/json")
        .insert_header((header::ETAG, quoted.as_str()))
        .insert_header((header::CACHE_CONTROL, cache_control))
        .body(body)
}

fn request_if_none_match(request: &HttpRequest) -> Vec<String> {
    request
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .map(|token| token.trim_start_matches("W/").trim_matches('"').to_owned())
                .collect()
        })
        .unwrap_or_default()
}
