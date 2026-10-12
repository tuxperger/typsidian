//! История переходов одной группы (FR-010…FR-014, research.md §5).
//!
//! Записи добавляет только действие пользователя, делающее активной другую
//! заметку группы (или тот же путь с явным местом назначения); «Назад» и
//! «Вперёд» двигают указатель и записей не добавляют. Тип [`History`] объявлен
//! в `mod.rs`, здесь — его правила.

use super::{Direction, History, HistoryEntry, TextPlace, HISTORY_LIMIT};

impl History {
    /// Текущая запись.
    pub fn current(&self) -> Option<&HistoryEntry> {
        self.entries.get(self.cursor)
    }

    /// Запоминает место, с которого ушли, в текущей записи — если она про ту
    /// же заметку. Иначе (запись про другую заметку) ничего не делает.
    pub(crate) fn note_leaving(&mut self, from: &str, leaving: Option<TextPlace>) {
        if let (Some(entry), Some(place)) = (self.entries.get_mut(self.cursor), leaving) {
            if entry.path == from {
                entry.place = Some(place);
            }
        }
    }

    /// Запись перехода `from → to`.
    ///
    /// `from` — заметка, с которой уходят (активная вкладка группы до
    /// действия), `leaving` — место каретки на ней. Переход без смены заметки
    /// и без явного места назначения — не переход (щелчок по уже активной
    /// вкладке): ничего не записывается.
    pub(crate) fn visit(
        &mut self,
        from: Option<&str>,
        leaving: Option<TextPlace>,
        to: HistoryEntry,
    ) {
        if let Some(from_path) = from {
            if from_path == to.path && to.place.is_none() {
                return;
            }
            self.sync_current(from_path, leaving);
        }
        if !self.entries.is_empty() {
            self.entries.truncate(self.cursor + 1);
        }
        // Соседние одинаковые записи схлопываются.
        if self.entries.last() != Some(&to) {
            self.entries.push(to);
        }
        self.cursor = self.entries.len() - 1;
        let excess = self.entries.len().saturating_sub(HISTORY_LIMIT);
        if excess > 0 {
            self.entries.drain(..excess);
            self.cursor -= excess;
        }
    }

    /// Гарантирует, что текущая запись — про `from`, и обновляет в ней место.
    /// Нужна после восстановления набора (история пуста, а вкладка уже
    /// активна) и после переноса вкладки между группами.
    fn sync_current(&mut self, from: &str, leaving: Option<TextPlace>) {
        match self.entries.get_mut(self.cursor) {
            Some(entry) if entry.path == from => {
                if leaving.is_some() {
                    entry.place = leaving;
                }
            }
            _ => {
                if !self.entries.is_empty() {
                    self.entries.truncate(self.cursor + 1);
                }
                self.entries.push(HistoryEntry {
                    path: from.to_owned(),
                    place: leaving,
                });
                self.cursor = self.entries.len() - 1;
            }
        }
    }

    /// Пересобирает записи: `map` может переписать путь или убрать запись.
    /// Соседние одинаковые схлопываются, указатель остаётся на той же записи
    /// (а если её убрали — на ближайшей предыдущей).
    fn rebuild(&mut self, mut map: impl FnMut(HistoryEntry) -> Option<HistoryEntry>) {
        let old = std::mem::take(&mut self.entries);
        let mut kept: Vec<HistoryEntry> = Vec::with_capacity(old.len());
        let mut up_to_cursor = 0;
        for (index, entry) in old.into_iter().enumerate() {
            if let Some(entry) = map(entry) {
                if kept.last() != Some(&entry) {
                    kept.push(entry);
                }
            }
            if index <= self.cursor {
                up_to_cursor = kept.len();
            }
        }
        self.cursor = up_to_cursor.saturating_sub(1);
        self.entries = kept;
    }

    /// Переименование: путь во всех записях (FR-013).
    pub(crate) fn rename(&mut self, from: &str, to: &str) {
        self.rebuild(|mut entry| {
            if entry.path == from {
                entry.path = to.to_owned();
            }
            Some(entry)
        });
    }

    /// Удаление: записи пути вычищаются (FR-013).
    pub(crate) fn purge(&mut self, path: &str) {
        self.rebuild(|entry| (entry.path != path).then_some(entry));
    }

    /// Индекс записи, на которую приведёт шаг, — первая в сторону движения,
    /// чья заметка ещё существует (`exists`). Удалённые пропускаются.
    ///
    /// Из пустой группы (все вкладки закрыты) «Назад» возвращает последнюю
    /// закрытую — текущую запись; «Вперёд» работает как обычно.
    pub(crate) fn step(
        &self,
        direction: Direction,
        group_is_empty: bool,
        exists: impl Fn(&str) -> bool,
    ) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        let candidates: Vec<usize> = match direction {
            Direction::Previous => {
                let start = if group_is_empty {
                    Some(self.cursor)
                } else {
                    self.cursor.checked_sub(1)
                };
                match start {
                    Some(start) => (0..=start).rev().collect(),
                    None => Vec::new(),
                }
            }
            Direction::Next => ((self.cursor + 1)..self.entries.len()).collect(),
        };
        candidates
            .into_iter()
            .find(|&index| exists(&self.entries[index].path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> HistoryEntry {
        HistoryEntry {
            path: path.to_owned(),
            place: None,
        }
    }

    #[test]
    fn purge_keeps_the_cursor_on_the_same_entry() {
        let mut history = History {
            entries: vec![entry("a"), entry("b"), entry("c")],
            cursor: 2,
        };
        history.purge("a");
        assert_eq!(history.entries, vec![entry("b"), entry("c")]);
        assert_eq!(history.current(), Some(&entry("c")));

        // Убрана запись под указателем — указатель на ближайшей предыдущей.
        history.purge("c");
        assert_eq!(history.current(), Some(&entry("b")));
        history.purge("b");
        assert!(history.entries.is_empty());
        assert_eq!(history.cursor, 0);
    }

    #[test]
    fn purge_collapses_the_neighbours_it_made_equal() {
        let mut history = History {
            entries: vec![entry("a"), entry("x"), entry("a"), entry("b")],
            cursor: 3,
        };
        history.purge("x");
        assert_eq!(history.entries, vec![entry("a"), entry("b")]);
        assert_eq!(history.current(), Some(&entry("b")));
    }

    #[test]
    fn the_limit_drops_the_oldest_entries() {
        let mut history = History::default();
        for n in 0..(HISTORY_LIMIT + 10) {
            history.visit(None, None, entry(&format!("n{n}")));
        }
        assert_eq!(history.entries.len(), HISTORY_LIMIT);
        assert_eq!(history.cursor, HISTORY_LIMIT - 1);
        assert_eq!(history.entries[0], entry("n10"));
    }
}
