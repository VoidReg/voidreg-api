use actix_web::{get, web};
use serde::Deserialize;

use crate::db::Db;
use crate::domain::ProjectListResponse;
use crate::error::AppError;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::IntoParams))]
pub struct ProjectQuery {
    pub featured: Option<bool>,
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/projects",
    tag = "projects",
    params(ProjectQuery),
    responses((status = 200, description = "Project list", body = ProjectListResponse))
))]
#[get("/projects")]
pub async fn list_projects(
    db: web::Data<Db>,
    query: web::Query<ProjectQuery>,
) -> Result<web::Json<ProjectListResponse>, AppError> {
    let items = db.list_projects(query.featured).await?;
    Ok(web::Json(ProjectListResponse { items }))
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/api/v1/projects/{slug}",
    tag = "projects",
    params(("slug" = String, Path, description = "Project slug")),
    responses(
        (status = 200, description = "Project detail", body = crate::domain::Project),
        (status = 404, description = "Project not found", body = crate::error::ErrorBody)
    )
))]
#[get("/projects/{slug}")]
pub async fn get_project(
    db: web::Data<Db>,
    path: web::Path<String>,
) -> Result<web::Json<crate::domain::Project>, AppError> {
    let project = db.get_project(&path).await?;
    Ok(web::Json(project))
}
