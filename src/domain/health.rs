use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "swagger", derive(utoipa::ToSchema))]
pub struct HealthResponse {
    pub status: &'static str,
}
