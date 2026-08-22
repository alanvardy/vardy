//! Stores all the environment variables and verifies that they are available at startup
//! Set them for production with `fly secrets set KEY=VALUE`
//! Set them locally in `.env`

pub struct Env {
    pub unsplash_api_key: String,
    pub database_url: String,
    // Read by Sentry init
    pub sentry_dsn: String,
    pub enable_sentry: bool,
}

impl Env {
    pub fn init() -> Env {
        let unsplash_api_key = get_string_env("UNSPLASH_API_KEY");
        let database_url = get_string_env("DATABASE_URL");
        let sentry_dsn = get_string_env("SENTRY_DSN");
        let enable_sentry = get_bool_env("ENABLE_SENTRY");

        Env {
            unsplash_api_key,
            database_url,
            sentry_dsn,
            enable_sentry,
        }
    }
}

fn get_string_env(key: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|var| !var.is_empty())
        .unwrap_or_else(|| panic!("{key} must be set and non-empty"))
}

fn get_bool_env(key: &str) -> bool {
    match get_string_env(key).as_str() {
        "true" => true,
        "false" => false,
        other => panic!("{key} must be 'true' or 'false', got '{other}'"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize env-var tests so they don't race with each other
    // when nextest runs tests in parallel.  Unwrapping poisoned
    // mutexes is safe here because the only failure mode inside a
    // critical section is a deliberate panic, and the next test
    // just needs to set/remove the same key.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    const TEST_KEY: &str = "TEST_GET_ENV_KEY";
    const BOOL_KEY: &str = "TEST_GET_BOOL_KEY";

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn get_env_returns_value_when_set_and_non_empty() {
        let _guard = lock();
        unsafe { std::env::set_var(TEST_KEY, "hello") };
        let result = get_string_env(TEST_KEY);
        unsafe { std::env::remove_var(TEST_KEY) };
        assert_eq!(result, "hello");
    }

    #[test]
    #[should_panic(expected = "must be set and non-empty")]
    fn get_env_panics_when_var_is_empty() {
        let _guard = lock();
        unsafe { std::env::set_var(TEST_KEY, "") };
        get_string_env(TEST_KEY);
    }

    #[test]
    #[should_panic(expected = "must be set and non-empty")]
    fn get_env_panics_when_var_is_missing() {
        let _guard = lock();
        unsafe { std::env::remove_var(TEST_KEY) };
        get_string_env(TEST_KEY);
    }

    #[test]
    fn get_bool_var_true() {
        let _guard = lock();
        unsafe { std::env::set_var(BOOL_KEY, "true") };
        let result = get_bool_env(BOOL_KEY);
        unsafe { std::env::remove_var(BOOL_KEY) };
        assert!(result);
    }

    #[test]
    fn get_bool_var_false() {
        let _guard = lock();
        unsafe { std::env::set_var(BOOL_KEY, "false") };
        let result = get_bool_env(BOOL_KEY);
        unsafe { std::env::remove_var(BOOL_KEY) };
        assert!(!result);
    }

    #[test]
    #[should_panic(expected = "must be 'true' or 'false'")]
    fn get_bool_var_panics_on_invalid() {
        let _guard = lock();
        unsafe { std::env::set_var(BOOL_KEY, "yes") };
        get_bool_env(BOOL_KEY);
    }
}
