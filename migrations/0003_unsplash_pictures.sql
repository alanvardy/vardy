CREATE TABLE IF NOT EXISTS unsplash_pictures (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url TEXT NOT NULL,
  photographer TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  photographer_url TEXT NOT NULL DEFAULT ''
);