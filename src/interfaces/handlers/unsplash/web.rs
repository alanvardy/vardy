use axum::{Json, extract::State};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::app::error::WebError;
use crate::app::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct Picture {
    pub url: String,
    pub photographer: String,
    pub created_at: String,
}

pub async fn index(State(state): State<AppState>) -> Result<Json<Picture>, WebError> {
    match latest_picture(&state.db).await? {
        Some(picture) => Ok(Json(picture)),
        None => Err(WebError::NotFound),
    }
}

async fn latest_picture(pool: &SqlitePool) -> Result<Option<Picture>, WebError> {
    let picture = sqlx::query_as::<_, Picture>(
        "SELECT url, photographer, created_at FROM unsplash_pictures ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(picture)
}

/// Used by Phase 3's read-through flow; until then only tests call it.
#[cfg_attr(not(test), allow(dead_code))]
async fn insert_picture(pool: &SqlitePool, picture: &Picture) -> Result<Picture, WebError> {
    let inserted = sqlx::query_as::<_, Picture>(
        "INSERT INTO unsplash_pictures (url, photographer) VALUES (?, ?) \
         RETURNING url, photographer, created_at",
    )
    .bind(&picture.url)
    .bind(&picture.photographer)
    .fetch_one(pool)
    .await?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use sqlx::Row;

    use super::*;
    use crate::test::{start_app, start_app_with, test_client};

    #[sqlx::test]
    async fn unsplash_pictures_table_exists(pool: SqlitePool) {
        let row = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'unsplash_pictures'",
        )
        .fetch_one(&pool)
        .await
        .expect("unsplash_pictures table should exist after migrations");
        assert_eq!(row.get::<String, _>("name"), "unsplash_pictures");
    }

    #[sqlx::test]
    async fn insert_picture_returns_row_with_created_at(pool: SqlitePool) {
        let picture = Picture {
            url: "https://example.com/x.jpg".to_string(),
            photographer: "Someone".to_string(),
            created_at: String::new(),
        };
        let inserted = insert_picture(&pool, &picture)
            .await
            .expect("insert should succeed");
        assert!(!inserted.created_at.is_empty());

        let latest = latest_picture(&pool)
            .await
            .expect("query should succeed")
            .expect("row should exist");
        assert_eq!(latest.url, "https://example.com/x.jpg");
        assert_eq!(latest.photographer, "Someone");
        assert_eq!(latest.created_at, inserted.created_at);
    }

    #[tokio::test]
    async fn unsplash_serves_seeded_row() {
        let (addr, db) = start_app_with("https://api.unsplash.com").await;
        sqlx::query("INSERT INTO unsplash_pictures (url, photographer) VALUES (?, ?)")
            .bind("https://example.com/seeded.jpg")
            .bind("Seeded Photographer")
            .execute(&db)
            .await
            .expect("seed insert should succeed");

        let client = test_client();
        let res = client
            .get(format!("http://{addr}/unsplash"))
            .send()
            .await
            .expect("request to /unsplash should succeed");
        assert_eq!(res.status(), 200);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("application/json"))
        );
        let body = res.text().await.expect("body");
        assert!(body.contains("https://example.com/seeded.jpg"));
        assert!(body.contains("Seeded Photographer"));
    }

    #[tokio::test]
    async fn unsplash_returns_404_when_empty() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/unsplash"))
            .send()
            .await
            .expect("request to /unsplash should succeed");
        assert_eq!(res.status(), 404);
    }
}
