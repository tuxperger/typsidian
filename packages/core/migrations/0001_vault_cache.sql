CREATE TABLE IF NOT EXISTS notes (
    path TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    mtime_millis INTEGER NOT NULL,
    size INTEGER NOT NULL,
    content_hash BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS links (
    source_path TEXT NOT NULL,
    target_path TEXT NOT NULL,
    PRIMARY KEY (source_path, target_path)
);

CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_path);
