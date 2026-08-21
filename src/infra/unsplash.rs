use crate::app::{error::WebError, picture::Picture};
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct RandomPhotoResponse {
    urls: RandomPhotoUrls,
    user: RandomPhotoUser,
}

#[derive(Deserialize)]
struct RandomPhotoUrls {
    regular: String,
}

#[derive(Deserialize)]
struct RandomPhotoUser {
    name: String,
}

/// Fetch a random nature photo from the Unsplash API.
/// Non-2xx status or parse failure maps to `WebError::External` (HTTP 502).
pub async fn fetch_random(
    client: &Client,
    base_url: &str,
    api_key: &str,
) -> Result<Picture, WebError> {
    let response = client
        .get(format!("{base_url}/photos/random"))
        .query(&[("query", "nature")])
        .header("Authorization", format!("Client-ID {api_key}"))
        .send()
        .await
        .map_err(|e| WebError::External(format!("unsplash request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(WebError::External(format!(
            "unsplash returned status {}",
            response.status()
        )));
    }

    let body: RandomPhotoResponse = response
        .json()
        .await
        .map_err(|e| WebError::External(format!("unsplash response parse failed: {e}")))?;

    Ok(Picture {
        url: body.urls.regular,
        photographer: body.user.name,
        created_at: String::new(), // populated by the DB on insert
    })
}
