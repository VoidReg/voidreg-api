use actix_web::{post, web};

use crate::config::Config;
use crate::db::Db;
use crate::domain::{ContactRequest, ContactResponse};
use crate::error::AppError;
use crate::mail;

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/api/v1/contact",
    tag = "contact",
    request_body = ContactRequest,
    responses(
        (status = 200, description = "Message accepted", body = ContactResponse),
        (status = 422, description = "Invalid payload", body = crate::error::ErrorBody),
        (status = 500, description = "Email delivery failed", body = crate::error::ErrorBody)
    )
))]
#[post("/contact")]
pub async fn create_contact(
    db: web::Data<Db>,
    config: web::Data<Config>,
    payload: web::Json<ContactRequest>,
) -> Result<web::Json<ContactResponse>, AppError> {
    let name = payload.name.trim();
    let email = payload.email.trim();
    let message = payload.message.trim();

    if name.is_empty() || name.len() > 120 {
        return Err(AppError::Unprocessable(
            "name must be between 1 and 120 characters".to_owned(),
        ));
    }
    if !is_valid_email(email) {
        return Err(AppError::Unprocessable(
            "enter a valid email address".to_owned(),
        ));
    }
    if message.is_empty() || message.len() > 8000 {
        return Err(AppError::Unprocessable(
            "message must be between 1 and 8000 characters".to_owned(),
        ));
    }

    mail::send_contact(&config, name, email, message).await?;
    db.insert_contact(name, email, message).await?;
    Ok(web::Json(ContactResponse { status: "received" }))
}

pub fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.contains(char::is_whitespace)
}
