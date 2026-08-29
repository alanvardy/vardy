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

impl ContactForm {
    /// Maximum name length in Unicode scalar values (characters).
    pub const MAX_NAME_CHARS: usize = 200;
    /// Maximum email length in bytes (RFC 5321).
    pub const MAX_EMAIL_BYTES: usize = 254;
    /// Maximum message length in Unicode scalar values (characters).
    pub const MAX_MESSAGE_CHARS: usize = 10_000;

    /// Validate form fields and return the first human-readable error, if any.
    /// Checks empty/whitespace fields first, then maximum lengths.
    /// Name and message lengths are measured in characters (`chars().count()`);
    /// email length is measured in bytes (`len()`, per RFC 5321).
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty() {
            return Err("Please enter your name.");
        }
        if self.email.trim().is_empty() {
            return Err("Please enter your email address.");
        }
        if self.message.trim().is_empty() {
            return Err("Please enter a message.");
        }
        if self.name.chars().count() > Self::MAX_NAME_CHARS {
            return Err("Name must be 200 characters or fewer.");
        }
        if self.email.len() > Self::MAX_EMAIL_BYTES {
            return Err("Email must be 254 characters or fewer.");
        }
        if self.message.chars().count() > Self::MAX_MESSAGE_CHARS {
            return Err("Message must be 10,000 characters or fewer.");
        }
        Ok(())
    }

    /// Build the email subject line.
    pub fn subject(&self) -> String {
        format!("New contact message from {} <{}>", self.name, self.email)
    }

    /// Build the email body text.
    pub fn body(&self) -> String {
        format!(
            "Name: {}\nEmail: {}\n\n{}",
            self.name, self.email, self.message
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn form(name: &str, email: &str, message: &str) -> ContactForm {
        ContactForm {
            name: name.into(),
            email: email.into(),
            message: message.into(),
            _website: None,
        }
    }

    #[test]
    fn valid_form_passes_validation() {
        assert!(form("Alan", "a@b.cc", "hi").validate().is_ok());
    }

    #[test]
    fn empty_name_rejected() {
        let err = form("", "a@b.cc", "hi").validate().unwrap_err();
        assert!(err.contains("name"));
    }

    #[test]
    fn whitespace_only_name_rejected() {
        let err = form("   ", "a@b.cc", "hi").validate().unwrap_err();
        assert!(err.contains("name"));
    }

    #[test]
    fn empty_email_rejected() {
        let err = form("Alan", "", "hi").validate().unwrap_err();
        assert!(err.contains("email"));
    }

    #[test]
    fn empty_message_rejected() {
        let err = form("Alan", "a@b.cc", "").validate().unwrap_err();
        assert!(err.contains("message"));
    }

    #[test]
    fn name_too_long_rejected() {
        let long = "a".repeat(201);
        assert!(form(&long, "a@b.cc", "hi").validate().is_err());
    }

    #[test]
    fn name_at_boundary_accepted() {
        let max = "a".repeat(200);
        assert!(form(&max, "a@b.cc", "hi").validate().is_ok());
    }

    #[test]
    fn email_too_long_rejected() {
        let long = "a".repeat(255);
        assert!(form("Alan", &long, "hi").validate().is_err());
    }

    #[test]
    fn message_too_long_rejected() {
        let long = "a".repeat(10_001);
        assert!(form("Alan", "a@b.cc", &long).validate().is_err());
    }

    #[test]
    fn message_at_boundary_accepted() {
        let max = "a".repeat(10_000);
        assert!(form("Alan", "a@b.cc", &max).validate().is_ok());
    }
}
