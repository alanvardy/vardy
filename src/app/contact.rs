use serde::Deserialize;

use crate::app::error::WebError;
use crate::app::state::AppState;
use crate::infra::resend;

#[derive(Deserialize)]
pub struct ContactForm {
    pub name: String,
    pub email: String,
    pub message: String,
    /// Honeypot: `None`/empty for humans, non-empty means a bot filled it.
    pub _website: Option<String>,
}

/// Send a contact email using the shared HTTP client and Resend config
/// carried on `AppState`.
pub async fn send(
    state: &AppState,
    from: &str,
    to: &str,
    subject: &str,
    text: &str,
) -> Result<(), WebError> {
    resend::send_contact_email(
        &state.http,
        &state.resend_base_url,
        &state.env.resend_api_key,
        from,
        to,
        subject,
        text,
    )
    .await?;
    Ok(())
}
