//! Private HTTP bridge for the MCP service. Nginx denies public access.
//! Bearer credentials never grant cookie-admin access or publishing rights.
use crate::{
    auth::{hash_token, rate::RateLimiter},
    config::Config,
    db::Db,
    domain::ArticleWriteRequest,
    error::AppError,
};
use actix_web::{HttpRequest, HttpResponse, get, post, put, web};
use serde::Deserialize;

pub async fn authorize(
    request: &HttpRequest,
    db: &Db,
    config: &Config,
    limiter: &RateLimiter,
) -> Result<(), AppError> {
    let token = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .filter(|s| s.starts_with("vr_mcp_") && s.len() <= 128)
        .ok_or_else(|| AppError::Unauthorized("MCP bearer token required".into()))?;
    let hash = hash_token(token);
    let identity = db
        .mcp_token_identity(&hash)
        .await?
        .ok_or_else(|| AppError::Unauthorized("MCP token expired or revoked".into()))?;
    let (provider, subject) = db
        .identity_subject(&identity)
        .await?
        .ok_or_else(|| AppError::Unauthorized("MCP owner no longer exists".into()))?;
    if provider != "github" || !config.admin_identity_allowed(&provider, &subject) {
        return Err(AppError::Forbidden(
            "MCP owner is no longer an administrator".into(),
        ));
    }
    if !limiter.check(
        &format!("mcp:{hash}"),
        240,
        std::time::Duration::from_secs(60),
    ) {
        return Err(AppError::TooManyRequests(
            "MCP request limit reached".into(),
        ));
    }
    db.touch_mcp_token(&hash).await?;
    Ok(())
}
#[get("/session")]
async fn session(
    request: HttpRequest,
    db: web::Data<Db>,
    config: web::Data<Config>,
    limiter: web::Data<RateLimiter>,
) -> Result<HttpResponse, AppError> {
    authorize(&request, &db, &config, &limiter).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"authenticated": true})))
}
#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
    page: Option<usize>,
    per_page: Option<usize>,
}
#[get("/articles")]
async fn list(
    request: HttpRequest,
    db: web::Data<Db>,
    config: web::Data<Config>,
    limiter: web::Data<RateLimiter>,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, AppError> {
    authorize(&request, &db, &config, &limiter).await?;
    if query
        .status
        .as_deref()
        .is_some_and(|s| s != "draft" && s != "published")
    {
        return Err(AppError::Unprocessable(
            "status must be draft or published".into(),
        ));
    }
    let items: Vec<_> = db
        .admin_list_articles()
        .await?
        .into_iter()
        .filter(|a| {
            query
                .status
                .as_deref()
                .is_none_or(|s| a.status.as_str() == s)
        })
        .collect();
    let total = items.len();
    let page = query.page.unwrap_or(1).clamp(1, 1_000_000);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
    let items: Vec<_> = items
        .into_iter()
        .skip((page - 1) * per_page)
        .take(per_page)
        .collect();
    Ok(HttpResponse::Ok()
        .json(serde_json::json!({"items":items,"page":page,"per_page":per_page,"total":total})))
}
#[get("/articles/{slug}")]
async fn get(
    request: HttpRequest,
    db: web::Data<Db>,
    config: web::Data<Config>,
    limiter: web::Data<RateLimiter>,
    slug: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    authorize(&request, &db, &config, &limiter).await?;
    Ok(HttpResponse::Ok().json(db.admin_get_article(&slug).await?))
}
fn restrict(input: &ArticleWriteRequest) -> Result<(), AppError> {
    if input.featured.is_some()
        || input.sort_order.is_some()
        || input.reading_time_minutes.is_some()
    {
        return Err(AppError::Forbidden(
            "MCP may edit title, category, summary, and draft content only".into(),
        ));
    }
    Ok(())
}
#[post("/articles")]
async fn create(
    request: HttpRequest,
    db: web::Data<Db>,
    config: web::Data<Config>,
    limiter: web::Data<RateLimiter>,
    input: web::Json<ArticleWriteRequest>,
) -> Result<HttpResponse, AppError> {
    authorize(&request, &db, &config, &limiter).await?;
    restrict(&input)?;
    super::admin::articles::create(&db, input.into_inner()).await
}
#[put("/articles/{slug}")]
async fn update(
    request: HttpRequest,
    db: web::Data<Db>,
    config: web::Data<Config>,
    limiter: web::Data<RateLimiter>,
    slug: web::Path<String>,
    input: web::Json<ArticleWriteRequest>,
) -> Result<HttpResponse, AppError> {
    authorize(&request, &db, &config, &limiter).await?;
    restrict(&input)?;
    super::admin::articles::update(&db, slug.into_inner(), input.into_inner(), true).await
}
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(session)
        .service(list)
        .service(get)
        .service(create)
        .service(update);
}
