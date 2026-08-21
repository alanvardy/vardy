use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Startup asset hashes, keyed by path relative to `static/`.
/// Lazily computed on first use (i.e., during `templates::init()`).
static ASSET_HASHES: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Hash every file under `dir` (recursive). Panics, naming the path, if the
/// directory or any file cannot be read — fail fast on broken deploys.
pub fn hash_all(dir: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    hash_dir(Path::new(dir), dir.len() + 1, &mut map);
    map
}

fn hash_dir(dir: &Path, prefix_len: usize, map: &mut HashMap<String, String>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to hash static asset {}: {err}", dir.display()));
    for entry in entries {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            hash_dir(&path, prefix_len, map);
        } else {
            let bytes = std::fs::read(&path).unwrap_or_else(|err| {
                panic!("failed to hash static asset {}: {err}", path.display())
            });
            let digest = Sha256::digest(&bytes);
            let rel = path.to_string_lossy()[prefix_len..].to_string();
            map.insert(rel, format!("{digest:x}")[..12].to_string());
        }
    }
}

/// `/static/<file>?v=<12-hex sha256 prefix>`. Panics on unknown files.
pub fn asset_url(file: &str) -> String {
    let hashes = ASSET_HASHES.get_or_init(|| hash_all("static"));
    match hashes.get(file) {
        Some(hash) => format!("/static/{file}?v={hash}"),
        None => panic!("unknown static asset {file}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_file_yields_versioned_url() {
        let url = asset_url("singlethread-icon.png");
        assert!(url.starts_with("/static/singlethread-icon.png?v="));
        let hash = url.rsplit("?v=").next().unwrap();
        assert_eq!(hash.len(), 12);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hashes_are_deterministic() {
        let a = hash_all("static");
        let b = hash_all("static");
        assert_eq!(a, b);
    }

    #[test]
    #[should_panic(expected = "failed to hash static asset")]
    fn unreadable_directory_panics() {
        hash_all("static/does-not-exist");
    }

    #[test]
    #[should_panic(expected = "unknown static asset")]
    fn unknown_file_panics() {
        // Ensure initialized even if another test ran first.
        let _ = ASSET_HASHES.get_or_init(|| hash_all("static"));
        asset_url("nope.css");
    }
}
