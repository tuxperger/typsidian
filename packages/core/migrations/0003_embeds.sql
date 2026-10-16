-- Файлы, вставленные в заметки вызовом `#image("…")` (фича 025, research.md §3).
-- Производное состояние: целиком восстанавливается пересчётом по текстам.
CREATE TABLE IF NOT EXISTS embeds (
    source_path TEXT NOT NULL,
    target_path TEXT NOT NULL,
    PRIMARY KEY (source_path, target_path)
);
