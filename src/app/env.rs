//! Stores environment variables and verifies that they are available at startup
//! Set them for production with `fly secrets set KEY=VALUE`
//! Set them locally in `.env`

pub struct Env {
    pub unsplash_api_key: String,
    pub database_url: String,
}

impl Env {
    pub fn init() -> Env {
        let unsplash_api_key = get_string_env("UNSPLASH_API_KEY");
        let database_url = get_string_env("DATABASE_URL");
        Env {
            unsplash_api_key,
            database_url,
        }
    }
}

fn get_string_env(key: &str) -> String {
    match std::env::var(key) {
        Ok(value) if !value.is_empty() => value,
        _ => panic!("Missing environment variable: {key}"),
    }
}
