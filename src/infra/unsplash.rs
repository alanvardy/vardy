use crate::domain::picture::Picture;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct RandomPhotoResponse {
    urls: RandomPhotoUrls,
    user: RandomPhotoUser,
}

#[derive(Deserialize, Debug)]
struct RandomPhotoUrls {
    regular: String,
}

#[derive(Deserialize, Debug)]
struct RandomPhotoUser {
    name: String,
    links: RandomPhotoUserLinks,
}

#[derive(Deserialize, Debug)]
struct RandomPhotoUserLinks {
    html: String,
}

/// Failure talking to the Unsplash API; translated into
/// `WebError::External` (HTTP 502) at the app layer.
#[derive(Debug)]
pub struct UnsplashError(pub String);

/// Fetch a random nature photo from the Unsplash API.
/// Non-2xx status or parse failure maps to `WebError::External` (HTTP 502)
/// via `From<UnsplashError>`.
pub async fn fetch_random(
    client: &Client,
    base_url: &str,
    api_key: &str,
) -> Result<Picture, UnsplashError> {
    let response = client
        .get(format!("{base_url}/photos/random"))
        .query(&[("query", "nature")])
        .header("Authorization", format!("Client-ID {api_key}"))
        .send()
        .await
        .map_err(|e| UnsplashError(format!("unsplash request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(UnsplashError(format!(
            "unsplash returned status {}",
            response.status()
        )));
    }

    let body: RandomPhotoResponse = response
        .json()
        .await
        .map_err(|e| UnsplashError(format!("unsplash response parse failed: {e}")))?;

    Ok(Picture {
        url: body.urls.regular,
        photographer: body.user.name,
        photographer_url: body.user.links.html,
        created_at: String::new(), // populated by the DB on insert
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_photographer_url_from_user_links_html() {
        let json = serde_json::json!({
            "urls": {"regular": "https://example.com/img.jpg"},
            "user": {
                "name": "Test Photographer",
                "links": {"html": "https://unsplash.com/@test"}
            }
        });
        let parsed: RandomPhotoResponse = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.user.links.html, "https://unsplash.com/@test");
    }

    #[test]
    fn missing_user_links_fails_parse() {
        let json = serde_json::json!({
            "urls": {"regular": "https://example.com/img.jpg"},
            "user": {"name": "Test Photographer"}
        });
        let err = serde_json::from_value::<RandomPhotoResponse>(json).unwrap_err();
        assert!(err.to_string().contains("links"));
    }
}
