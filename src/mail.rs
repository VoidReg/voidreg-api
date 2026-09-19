use serde_json::json;

use crate::config::Config;
use crate::error::AppError;

const RESEND_URL: &str = "https://api.resend.com/emails";

pub async fn send_contact(
    config: &Config,
    name: &str,
    reply_to: &str,
    message: &str,
) -> Result<(), AppError> {
    if config.skips_outbound_mail() {
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .user_agent("voidreg-api/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|err| AppError::Internal(err.to_string()))?;

    let response = client
        .post(RESEND_URL)
        .bearer_auth(&config.resend_api_key)
        .json(&json!({
            "from": config.resend_from(),
            "to": [config.contact_to_email.as_str()],
            "reply_to": reply_to,
            "subject": format!("Contact form: {name}"),
            "text": format!("From: {name} <{reply_to}>\n\n{message}"),
        }))
        .send()
        .await
        .map_err(|err| AppError::Internal(err.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let _ = response.text().await;
        tracing::error!(status = %status, "contact email send failed");
        return Err(AppError::Internal(
            "failed to send contact email".to_owned(),
        ));
    }

    Ok(())
}
