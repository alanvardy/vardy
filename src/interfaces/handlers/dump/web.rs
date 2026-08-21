use crate::app::error::WebError;
use crate::app::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
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
}
