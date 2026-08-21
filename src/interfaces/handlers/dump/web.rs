use crate::app::error::WebError;
use crate::app::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DumpEntry {
    pub id: i64,
    pub body: serde_json::Value,
}

pub async fn index(
    Path(key): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<DumpEntry>>, WebError> {
    let entries = sqlx::query_as!(
        DumpEntry,
        r#"SELECT id, body AS "body: serde_json::Value" FROM dumps WHERE key = ? ORDER BY id"#,
        key
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(entries))
}

pub async fn create(
    Path(key): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<StatusCode, WebError> {
    let serialized = serde_json::to_string(&body).expect("serializing Value cannot fail");
    sqlx::query!(
        "INSERT INTO dumps (key, body) VALUES (?, ?)",
        key,
        serialized
    )
    .execute(&state.db)
    .await?;
    Ok(StatusCode::CREATED)
}

#[cfg(test)]
mod tests {
    use crate::test::{start_app, test_client};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn get_unknown_key_returns_empty_list() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/dump/nope"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("application/json"))
        );
        let body = res.text().await.unwrap();
        assert_eq!(body, "[]");
    }

    #[tokio::test]
    async fn post_stores_and_get_returns_it() {
        let addr = start_app().await;
        let client = test_client();
        let payload = serde_json::json!({ "a": 1, "nested": { "b": [true, null] } });
        let res = client
            .post(format!("http://{addr}/dump/k"))
            .json(&payload)
            .send()
            .await
            .expect("request failed");
        assert!(res.status().is_success());

        let res = client
            .get(format!("http://{addr}/dump/k"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let entries: Vec<crate::interfaces::handlers::dump::web::DumpEntry> =
            res.json().await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, 1);
        assert_eq!(entries[0].body, payload);
    }

    #[tokio::test]
    async fn multiple_posts_accumulate() {
        let addr = start_app().await;
        let client = test_client();
        for n in 0..3 {
            client
                .post(format!("http://{addr}/dump/acc"))
                .json(&serde_json::json!({ "n": n }))
                .send()
                .await
                .expect("request failed");
        }
        let res = client
            .get(format!("http://{addr}/dump/acc"))
            .send()
            .await
            .expect("request failed");
        let entries: Vec<crate::interfaces::handlers::dump::web::DumpEntry> =
            res.json().await.unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries
                .iter()
                .map(|e| e.body["n"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            vec![0, 1, 2] // insertion order
        );
    }

    #[tokio::test]
    async fn post_invalid_json_rejected() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .post(format!("http://{addr}/dump/bad"))
            .header("content-type", "application/json")
            .body("{not json")
            .send()
            .await
            .expect("request failed");
        // axum 0.8: malformed JSON syntax -> 400 (JsonSyntaxError)
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
