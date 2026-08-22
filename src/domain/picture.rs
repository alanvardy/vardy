use serde::Serialize;

/// A picture served by the `/unsplash` endpoint, persisted in the
/// `unsplash_pictures` table.
#[derive(Serialize, sqlx::FromRow)]
pub struct Picture {
    pub url: String,
    pub photographer: String,
    pub created_at: String,
}
