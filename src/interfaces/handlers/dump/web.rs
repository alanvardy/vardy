use crate::app::dump::{self, DumpEntry};
use crate::app::error::WebError;
use crate::app::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

pub async fn index(
    Path(key): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<DumpEntry>>, WebError> {
    let entries = dump::list(&state.db, &key).await?;
    Ok(Json(entries))
}

pub async fn create(
    Path(key): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<StatusCode, WebError> {
    let serialized = serde_json::to_string(&body).expect("serializing Value cannot fail");
    dump::create(&state.db, &key, &serialized).await?;
    Ok(StatusCode::CREATED)
}

#[cfg(test)]
mod tests {
    use crate::test::{start_app, start_app_with_rate_limits, test_client};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn dump_post_tier_trips_while_global_budget_stays_open() {
        let (addr, _pool) =
            start_app_with_rate_limits("https://api.unsplash.com", 1, 1_000_000).await;
        let client = test_client();
        // DUMP_TIER_BURST = 3; fire 15 concurrent POSTs of tiny JSON
        let handles: Vec<_> = (0..15)
            .map(|_| {
                let client = client.clone();
                let url = format!("http://{addr}/dump/tier-test");
                tokio::spawn(async move {
                    client
                        .post(url)
                        .json(&serde_json::json!({ "n": 1 }))
                        .send()
                        .await
                        .expect("request failed")
                })
            })
            .collect();
        let mut created = 0;
        let mut limited = 0;
        for handle in handles {
            let res = handle.await.expect("join");
            match res.status() {
                StatusCode::CREATED => created += 1,
                StatusCode::TOO_MANY_REQUESTS => {
                    limited += 1;
                    assert!(res.headers().get("retry-after").is_some());
                    assert_eq!(res.text().await.unwrap(), "too many requests");
                }
                status => panic!("unexpected status {status}"),
            }
        }
        assert!(created >= 1, "at least one POST should be created");
        assert!(limited >= 5, "tier should trip well before global budget");
    }

    #[tokio::test]
    async fn dump_get_is_not_tier_limited() {
        let (addr, _pool) =
            start_app_with_rate_limits("https://api.unsplash.com", 1, 1_000_000).await;
        let client = test_client();
        // 30 sequential GETs to /dump/anything -> all 200 (would trip any sane tier)
        for _ in 0..30 {
            let res = client
                .get(format!("http://{addr}/dump/anything"))
                .send()
                .await
                .expect("request failed");
            assert_eq!(res.status(), StatusCode::OK);
        }
    }

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
        let entries: Vec<crate::app::dump::DumpEntry> = res.json().await.unwrap();
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
        let entries: Vec<crate::app::dump::DumpEntry> = res.json().await.unwrap();
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
