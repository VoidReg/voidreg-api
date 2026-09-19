use actix_web::{HttpResponse, get, web};

use crate::db::Db;
use crate::domain::HealthResponse;
use crate::error::AppError;

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/healthz",
    tag = "health",
    responses((status = 200, description = "Service is up", body = HealthResponse))
))]
#[get("/healthz")]
pub async fn healthz() -> web::Json<HealthResponse> {
    web::Json(HealthResponse { status: "ok" })
}

#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/readyz",
    tag = "health",
    responses(
        (status = 200, description = "Database is reachable", body = HealthResponse),
        (status = 500, description = "Database is unreachable", body = crate::error::ErrorBody)
    )
))]
#[get("/readyz")]
pub async fn readyz(db: web::Data<Db>) -> Result<HttpResponse, AppError> {
    db.ping().await?;
    Ok(HttpResponse::Ok().json(HealthResponse { status: "ok" }))
}
