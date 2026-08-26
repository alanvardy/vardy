use reqwest::Client;
use serde::Serialize;

#[derive(Serialize)]
struct SendEmailRequest {
    from: String,
    to: [String; 1],
    subject: String,
    text: String,
}

/// Failure talking to the Resend API; translated into
/// `WebError::External` (HTTP 502) at the app layer.
#[derive(Debug)]
pub struct ResendError(pub String);

/// Send a plain-text contact email through the Resend API.
/// Non-2xx status maps to `WebError::External` (HTTP 502) via
/// `From<ResendError>`.
pub async fn send_contact_email(
    client: &Client,
    base_url: &str,
    api_key: &str,
    from: &str,
    to: &str,
    subject: &str,
    text: &str,
) -> Result<(), ResendError> {
    let response = client
        .post(format!("{base_url}/emails"))
        .bearer_auth(api_key)
        .json(&SendEmailRequest {
            from: from.to_owned(),
            to: [to.to_owned()],
            subject: subject.to_owned(),
            text: text.to_owned(),
        })
        .send()
        .await
        .map_err(|e| ResendError(format!("resend request failed: {e}")))?;

    if !response.status().is_success() {
        // Capture the upstream error body (truncated) so 502s are diagnosable;
        // a non-2xx with no body yields an empty detail string.
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(500)
            .collect::<String>();
        return Err(ResendError(format!(
            "resend returned status {status}: {body}"
        )));
    }

    // A 2xx status already proves the API accepted the message; parsing the
    // body would only add a false-failure path (e.g. on an unexpected body).
    Ok(())
}
