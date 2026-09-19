use actix_web::{post, web};

use crate::api::v1::contact::is_valid_email;
use crate::db::Db;
use crate::domain::{NewsletterRequest, NewsletterResponse};
use crate::error::AppError;

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/api/v1/newsletter/subscribe",
    tag = "newsletter",
    request_body = NewsletterRequest,
    responses(
        (status = 200, description = "Subscription recorded", body = NewsletterResponse),
        (status = 422, description = "Invalid email", body = crate::error::ErrorBody)
    )
))]
#[post("/newsletter/subscribe")]
pub async fn subscribe(
    db: web::Data<Db>,
    payload: web::Json<NewsletterRequest>,
) -> Result<web::Json<NewsletterResponse>, AppError> {
    let email = payload.email.trim().to_lowercase();
    if !is_valid_email(&email) {
        return Err(AppError::Unprocessable(
            "enter a valid email address".to_owned(),
        ));
    }

    db.subscribe_newsletter(&email).await?;
    Ok(web::Json(NewsletterResponse {
        status: "subscribed",
        email,
    }))
}
