use actix_web::{get, web};
use serde::Deserialize;

use crate::db::Db;
use crate::domain::ArticleListResponse;
use crate::error::AppError;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::IntoParams))]
pub struct BlogQuery {
    pub featured: Option<bool>,
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/blogs",
    tag = "blogs",
    params(BlogQuery),
    responses((status = 200, description = "Blog list", body = ArticleListResponse))
))]
#[get("/blogs")]
pub async fn list_blogs(
    db: web::Data<Db>,
    query: web::Query<BlogQuery>,
) -> Result<web::Json<ArticleListResponse>, AppError> {
    let items = db.list_articles(query.featured).await?;
    Ok(web::Json(ArticleListResponse { items }))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/blogs/{slug}",
    tag = "blogs",
    params(("slug" = String, Path, description = "Blog slug")),
    responses(
        (status = 200, description = "Blog detail", body = crate::domain::Article),
        (status = 404, description = "Blog not found", body = crate::error::ErrorBody)
    )
))]
#[get("/blogs/{slug}")]
pub async fn get_blog(
    db: web::Data<Db>,
    path: web::Path<String>,
) -> Result<web::Json<crate::domain::Article>, AppError> {
    let article = db.get_article(&path).await?;
    Ok(web::Json(article))
}
