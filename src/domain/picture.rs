use serde::Serialize;

/// How long a cached picture stays fresh before a new one is fetched.
const MAX_AGE_HOURS: i64 = 6;

/// A picture served by the `/unsplash` endpoint, persisted in the
/// `unsplash_pictures` table.
#[derive(Serialize, sqlx::FromRow)]
pub struct Picture {
    pub url: String,
    pub photographer: String,
    pub photographer_url: String,
    pub created_at: String,
}

impl Picture {
    pub fn is_stale(&self) -> bool {
        chrono::NaiveDateTime::parse_from_str(&self.created_at, "%Y-%m-%d %H:%M:%S")
            .map(|created_at| {
                chrono::Utc::now().naive_utc() - created_at > chrono::Duration::hours(MAX_AGE_HOURS)
            })
            .unwrap_or(true) // unparseable timestamp → treat as stale, force refresh
    }
}
