-- Свойства заметок (фича 020, research.md §4). Производное состояние: целиком
-- восстанавливается пересчётом по текстам заметок.
CREATE TABLE IF NOT EXISTS properties (
    path TEXT NOT NULL,
    ord INTEGER NOT NULL,
    key TEXT NOT NULL,
    kind TEXT NOT NULL,
    value_text TEXT,
    value_num REAL
);

CREATE INDEX IF NOT EXISTS idx_properties_path ON properties(path);

-- Служебные сведения кэша: версия формата свойств.
CREATE TABLE IF NOT EXISTS cache_meta (
    name TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
