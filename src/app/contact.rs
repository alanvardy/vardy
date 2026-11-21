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

    /// Bot signal: a CR or LF in `name` or `email`.
    ///
    /// Both fields render as single-line inputs, so a line break is never
    /// legitimate there, and CRLF is the classic SMTP header-injection probe.
    /// `message` is deliberately excluded because multi-line messages are
    /// legitimate.
    pub fn looks_like_bot(&self) -> bool {
        self.name.contains(['\r', '\n']) || self.email.contains(['\r', '\n'])
    }

    /// Build the email subject line.
    ///
    /// `name` and `email` are user-supplied, so strip any control characters
    /// (including CR/LF) before interpolating: a bare newline in the subject
    /// makes Resend reject the send with HTTP 422. Clean input is unchanged.
    pub fn subject(&self) -> String {
        format!(
            "New contact message from {} <{}>",
            strip_control_chars(&self.name),
            strip_control_chars(&self.email)
        )
    }

    /// Build the email body text.
    pub fn body(&self) -> String {
        format!(
            "Name: {}\nEmail: {}\n\n{}",
            self.name, self.email, self.message
        )
    }
}

/// Remove control characters (CR, LF, NUL, etc.) so a value can be embedded in
/// a single-line email header. Non-control characters, including spaces and
/// Unicode, are preserved byte-for-byte.
fn strip_control_chars(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).collect()
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

    #[test]
    fn clean_input_does_not_look_like_bot() {
        assert!(!form("Alan", "a@b.cc", "hi").looks_like_bot());
    }

    #[test]
    fn newline_in_name_looks_like_bot() {
        assert!(form("Alan\nVardy", "a@b.cc", "hi").looks_like_bot());
    }

    #[test]
    fn carriage_return_in_email_looks_like_bot() {
        assert!(form("Alan", "a@b.cc\rX-Injected: 1", "hi").looks_like_bot());
    }

    #[test]
    fn newline_in_message_is_not_a_bot_signal() {
        assert!(!form("Alan", "a@b.cc", "line one\nline two").looks_like_bot());
    }

    #[test]
    fn subject_keeps_clean_input_unchanged() {
        assert_eq!(
            form("Alan", "a@b.cc", "hi").subject(),
            "New contact message from Alan <a@b.cc>"
        );
    }

    #[test]
    fn subject_strips_newlines_from_name() {
        let subject = form("Alan\nVardy", "a@b.cc", "hi").subject();
        assert_eq!(subject, "New contact message from AlanVardy <a@b.cc>");
        assert!(!subject.chars().any(char::is_control));
    }

    #[test]
    fn subject_strips_carriage_returns_from_name() {
        let subject = form("Alan\rVardy", "a@b.cc", "hi").subject();
        assert_eq!(subject, "New contact message from AlanVardy <a@b.cc>");
        assert!(!subject.chars().any(char::is_control));
    }

    #[test]
    fn subject_strips_newlines_from_email() {
        let subject = form("Alan", "a@b.cc\nX-Injected: 1", "hi").subject();
        assert_eq!(
            subject,
            "New contact message from Alan <a@b.ccX-Injected: 1>"
        );
        assert!(!subject.chars().any(char::is_control));
    }

    #[test]
    fn subject_strips_crlf_from_name_and_email() {
        let subject = form("Alan\r\nVardy", "a@b.cc\r\n", "hi").subject();
        assert_eq!(subject, "New contact message from AlanVardy <a@b.cc>");
        assert!(!subject.chars().any(char::is_control));
    }

    #[test]
    fn subject_strips_other_control_chars() {
        let subject = form("Alan\u{0}\u{7}Vardy", "a@b.cc", "hi").subject();
        assert_eq!(subject, "New contact message from AlanVardy <a@b.cc>");
        assert!(!subject.chars().any(char::is_control));
    }
}
