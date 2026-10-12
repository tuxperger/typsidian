use crate::vault::index::FileStamp;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Состояние `EditConflict` (data-model.md). Переход только по явному действию
/// пользователя — автоматический переход запрещён (FR-021, FR-023).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ConflictStatus {
    Pending,
    ResolvedKeepLocal,
    ResolvedAcceptExternal,
}

/// Конфликт редактирования: существует только в памяти сессии (FR-035) — не
/// персистентный, кэш не хранит `local_buffer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct EditConflict {
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub note_path: PathBuf,
    pub local_buffer: String,
    pub disk_snapshot: FileStamp,
    pub status: ConflictStatus,
}

impl EditConflict {
    pub fn new(note_path: PathBuf, local_buffer: String, disk_snapshot: FileStamp) -> Self {
        Self {
            note_path,
            local_buffer,
            disk_snapshot,
            status: ConflictStatus::Pending,
        }
    }

    /// Обновляет снимок диска, если конфликт ещё не разрешён и диск изменился
    /// повторно (US4 AS5) — не создаёт новый конфликт, актуализирует текущий.
    pub fn refresh_disk_snapshot(&mut self, new_snapshot: FileStamp) {
        if self.status == ConflictStatus::Pending {
            self.disk_snapshot = new_snapshot;
        }
    }
}

/// Первая ступень двухступенчатой проверки (research.md §5): дешёвый предфильтр
/// по метаданным. Отвечает на вопрос «стоит ли вообще читать файл», а не «есть
/// ли конфликт» — mtime правится некоторыми программами без реального изменения
/// содержимого, поэтому одного этого признака недостаточно.
pub fn looks_changed(known: &FileStamp, current: &FileStamp) -> bool {
    known.mtime_millis != current.mtime_millis || known.size != current.size
}

/// Вторая ступень (research.md §5): подтверждение хэшем содержимого. Именно
/// она решает, поднимать ли конфликт, — если содержимое на диске совпало с
/// последним известным, конфликта нет, пусть даже mtime изменился. Побочный
/// эффект того же правила: изменение файла с последующим возвратом прежнего
/// содержимого не даёт ложного конфликта.
pub fn is_meaningful_change(known: &FileStamp, current: &FileStamp) -> bool {
    known.content_hash != current.content_hash
}

/// Реестр несохранённых буферов и активных конфликтов (T071, T072). Живёт
/// целиком в памяти сессии (FR-035): ни буфер, ни конфликт не попадают в
/// персистентный кэш.
///
/// Реестр хранит **только неразрешённые** конфликты: `resolve` убирает запись,
/// а два терминальных варианта [`ConflictStatus`] описывают, каким именно
/// действием пользователя это произошло. Автоматических переходов нет ни
/// одного — все входные точки вызываются либо конвейером обнаружения
/// (поднять конфликт), либо явным действием пользователя (разрешить его).
#[derive(Debug, Default)]
pub struct ConflictRegistry {
    /// Несохранённое содержимое открытых в редакторе заметок.
    buffers: HashMap<PathBuf, String>,
    /// Неразрешённые конфликты «диск разошёлся с буфером» (FR-021).
    conflicts: HashMap<PathBuf, EditConflict>,
    /// Заметки, удалённые снаружи при наличии несохранённых правок (FR-025).
    /// Отдельно от `conflicts`, потому что у такой ситуации нет содержимого на
    /// диске, а `EditConflict::disk_snapshot` обязан описывать реально
    /// существующую версию.
    deleted_with_edits: HashSet<PathBuf>,
}

impl ConflictRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Редактор объявил несохранённые правки (или обновил их по мере ввода).
    pub fn set_buffer(&mut self, path: &Path, buffer: String) {
        self.buffers.insert(path.to_path_buf(), buffer);
    }

    /// Правки сохранены или отброшены самим редактором — снимает и буфер, и
    /// всё, что от него зависело.
    pub fn clear(&mut self, path: &Path) {
        self.buffers.remove(path);
        self.conflicts.remove(path);
        self.deleted_with_edits.remove(path);
    }

    /// Всё, что привязано к пути заметки, переезжает вместе с ней (фича 023,
    /// FR-020): буфер, неразрешённый конфликт и «удалено с правками». Для папки
    /// — всё под ней. Без этого открытая заметка после переноса потеряла бы
    /// несохранённые правки.
    ///
    /// Возвращает число переехавших записей (по всем трём таблицам).
    pub fn rename_prefix(&mut self, from: &Path, to: &Path) -> usize {
        let rebase = |path: &Path| super::relocate::rebase(from, to, path);
        let mut moved = 0;

        let buffers: Vec<PathBuf> = self
            .buffers
            .keys()
            .filter(|path| path.starts_with(from))
            .cloned()
            .collect();
        for old in buffers {
            if let Some(buffer) = self.buffers.remove(&old) {
                self.buffers.insert(rebase(&old), buffer);
                moved += 1;
            }
        }

        let conflicts: Vec<PathBuf> = self
            .conflicts
            .keys()
            .filter(|path| path.starts_with(from))
            .cloned()
            .collect();
        for old in conflicts {
            if let Some(mut conflict) = self.conflicts.remove(&old) {
                let new = rebase(&old);
                conflict.note_path = new.clone();
                self.conflicts.insert(new, conflict);
                moved += 1;
            }
        }

        let deleted: Vec<PathBuf> = self
            .deleted_with_edits
            .iter()
            .filter(|path| path.starts_with(from))
            .cloned()
            .collect();
        for old in deleted {
            self.deleted_with_edits.remove(&old);
            self.deleted_with_edits.insert(rebase(&old));
            moved += 1;
        }
        moved
    }

    pub fn has_unsaved_edits(&self, path: &Path) -> bool {
        self.buffers.contains_key(path)
    }

    pub fn buffer(&self, path: &Path) -> Option<&str> {
        self.buffers.get(path).map(String::as_str)
    }

    pub fn conflict(&self, path: &Path) -> Option<&EditConflict> {
        self.conflicts.get(path)
    }

    pub fn was_deleted_with_edits(&self, path: &Path) -> bool {
        self.deleted_with_edits.contains(path)
    }

    /// Пути, по которым есть что терять при закрытии окна (FR-037, FR-035):
    /// неразрешённый конфликт, внешнее удаление с правками — или просто
    /// несохранённый буфер.
    pub fn unresolved_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .buffers
            .keys()
            .chain(self.conflicts.keys())
            .chain(self.deleted_with_edits.iter())
            .cloned()
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    /// Конвейер обнаружения увидел изменение файла на диске. Возвращает `true`,
    /// если это конфликт (то есть у заметки есть несохранённые правки и
    /// содержимое действительно разошлось) — тогда вызывающая сторона обязана
    /// эмитить `ConflictDetected` вместо тихого `NoteUpdated` (FR-021).
    ///
    /// Повторное изменение диска до разрешения не создаёт второй конфликт, а
    /// актуализирует снимок существующего (US4 AS5).
    pub fn observe_disk_change(
        &mut self,
        path: &Path,
        known: &FileStamp,
        current: &FileStamp,
    ) -> bool {
        let Some(buffer) = self.buffers.get(path) else {
            return false;
        };
        if !is_meaningful_change(known, current) {
            return false;
        }
        match self.conflicts.get_mut(path) {
            Some(existing) => existing.refresh_disk_snapshot(current.clone()),
            None => {
                self.conflicts.insert(
                    path.to_path_buf(),
                    EditConflict::new(path.to_path_buf(), buffer.clone(), current.clone()),
                );
            }
        }
        true
    }

    /// Конвейер обнаружения увидел удаление файла снаружи. Возвращает `true`,
    /// если у заметки были несохранённые правки — тогда эмитится
    /// `NoteDeletedWithUnsavedEdits` вместо `NoteDeleted` (FR-025).
    pub fn observe_disk_deletion(&mut self, path: &Path) -> bool {
        if !self.buffers.contains_key(path) {
            return false;
        }
        // Более сильный факт вытесняет более слабый: файла больше нет, значит
        // снимок диска в `EditConflict` описывать нечему.
        self.conflicts.remove(path);
        self.deleted_with_edits.insert(path.to_path_buf());
        true
    }

    /// Явное действие пользователя, закрывающее конфликт (FR-023). Возвращает
    /// `false`, если разрешать было нечего.
    pub fn resolve(&mut self, path: &Path, outcome: ConflictStatus) -> bool {
        debug_assert_ne!(
            outcome,
            ConflictStatus::Pending,
            "resolve() фиксирует терминальный исход, а не возвращает конфликт в ожидание"
        );
        let had_conflict = self.conflicts.remove(path).is_some();
        let had_deletion = self.deleted_with_edits.remove(path);
        let had_buffer = self.buffers.remove(path).is_some();
        had_conflict || had_deletion || had_buffer
    }
}

/// Верхняя граница построчного сопоставления для diff (FR-024). Матрица LCS
/// квадратична по числу строк, поэтому на заметках неправдоподобного размера
/// сопоставление заменяется честным показом обеих сторон целиком — лучше
/// огрублённое отличие, чем зависший интерфейс.
const DIFF_LINE_BUDGET: usize = 2000;

/// Строка diff между локальным буфером и версией на диске (FR-024).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "side")]
pub enum DiffLine {
    /// Строка совпала у обеих сторон.
    Same { text: String },
    /// Строка есть только в версии пользователя.
    Local { text: String },
    /// Строка есть только в версии на диске.
    External { text: String },
}

/// Построчный diff между версией пользователя и версией на диске (T073,
/// FR-024). Сам по себе ничего не разрешает: просмотр отличий возвращает
/// пользователя к выбору из трёх вариантов.
pub fn line_diff(local: &str, external: &str) -> Vec<DiffLine> {
    let local_lines: Vec<&str> = local.lines().collect();
    let external_lines: Vec<&str> = external.lines().collect();

    if local_lines.len() > DIFF_LINE_BUDGET || external_lines.len() > DIFF_LINE_BUDGET {
        return local_lines
            .iter()
            .map(|text| DiffLine::Local {
                text: (*text).to_string(),
            })
            .chain(external_lines.iter().map(|text| DiffLine::External {
                text: (*text).to_string(),
            }))
            .collect();
    }

    let n = local_lines.len();
    let m = external_lines.len();
    // lcs[i * (m + 1) + j] — длина наибольшей общей подпоследовательности
    // суффиксов local[i..] и external[j..]. Плоский `u32`-буфер вместо
    // `Vec<Vec<usize>>` — вчетверо меньше памяти на том же бюджете строк.
    let mut lcs = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let value = if local_lines[i] == external_lines[j] {
                lcs[(i + 1) * (m + 1) + (j + 1)] + 1
            } else {
                lcs[(i + 1) * (m + 1) + j].max(lcs[i * (m + 1) + (j + 1)])
            };
            lcs[i * (m + 1) + j] = value;
        }
    }

    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if local_lines[i] == external_lines[j] {
            out.push(DiffLine::Same {
                text: local_lines[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if lcs[(i + 1) * (m + 1) + j] >= lcs[i * (m + 1) + (j + 1)] {
            out.push(DiffLine::Local {
                text: local_lines[i].to_string(),
            });
            i += 1;
        } else {
            out.push(DiffLine::External {
                text: external_lines[j].to_string(),
            });
            j += 1;
        }
    }
    while i < n {
        out.push(DiffLine::Local {
            text: local_lines[i].to_string(),
        });
        i += 1;
    }
    while j < m {
        out.push(DiffLine::External {
            text: external_lines[j].to_string(),
        });
        j += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(content: &str, mtime: i64) -> FileStamp {
        FileStamp::from_content(mtime, content.as_bytes())
    }

    #[test]
    fn touched_file_with_identical_content_is_not_a_conflict() {
        // research.md §5: mtime изменился, содержимое — нет.
        let known = stamp("одно и то же", 100);
        let current = stamp("одно и то же", 900);
        assert!(looks_changed(&known, &current));
        assert!(!is_meaningful_change(&known, &current));
    }

    #[test]
    fn conflict_is_raised_only_when_note_has_unsaved_edits() {
        let mut registry = ConflictRegistry::new();
        let known = stamp("исходное", 100);
        let current = stamp("внешнее", 200);
        let path = Path::new("note.typ");

        assert!(!registry.observe_disk_change(path, &known, &current));

        registry.set_buffer(path, "мои правки".to_string());
        assert!(registry.observe_disk_change(path, &known, &current));
        assert_eq!(
            registry.conflict(path).map(|c| c.status),
            Some(ConflictStatus::Pending)
        );
    }

    #[test]
    fn repeated_disk_change_refreshes_snapshot_without_second_conflict() {
        let mut registry = ConflictRegistry::new();
        let path = Path::new("note.typ");
        registry.set_buffer(path, "мои правки".to_string());

        let first = stamp("первое внешнее", 200);
        registry.observe_disk_change(path, &stamp("исходное", 100), &first);
        let second = stamp("второе внешнее", 300);
        registry.observe_disk_change(path, &first, &second);

        let conflict = registry.conflict(path).expect("конфликт должен быть один");
        assert_eq!(conflict.disk_snapshot, second);
        assert_eq!(conflict.local_buffer, "мои правки");
    }

    #[test]
    fn resolution_clears_buffer_and_conflict() {
        let mut registry = ConflictRegistry::new();
        let path = Path::new("note.typ");
        registry.set_buffer(path, "мои правки".to_string());
        registry.observe_disk_change(path, &stamp("a", 1), &stamp("b", 2));

        assert!(registry.resolve(path, ConflictStatus::ResolvedAcceptExternal));
        assert!(!registry.has_unsaved_edits(path));
        assert!(registry.conflict(path).is_none());
        assert!(registry.unresolved_paths().is_empty());
        assert!(!registry.resolve(path, ConflictStatus::ResolvedAcceptExternal));
    }

    #[test]
    fn external_deletion_is_tracked_separately_from_content_conflict() {
        let mut registry = ConflictRegistry::new();
        let path = Path::new("note.typ");

        assert!(!registry.observe_disk_deletion(path));

        registry.set_buffer(path, "не потерять".to_string());
        registry.observe_disk_change(path, &stamp("a", 1), &stamp("b", 2));
        assert!(registry.observe_disk_deletion(path));

        assert!(registry.was_deleted_with_edits(path));
        assert!(
            registry.conflict(path).is_none(),
            "снимку диска нечего описывать, если файла больше нет"
        );
        assert_eq!(registry.buffer(path), Some("не потерять"));
    }

    #[test]
    fn diff_marks_common_lines_and_both_sides_of_a_change() {
        let diff = line_diff("общая\nмоя\nхвост\n", "общая\nвнешняя\nхвост\n");
        assert!(diff.contains(&DiffLine::Same {
            text: "общая".to_string()
        }));
        assert!(diff.contains(&DiffLine::Local {
            text: "моя".to_string()
        }));
        assert!(diff.contains(&DiffLine::External {
            text: "внешняя".to_string()
        }));
        assert!(diff.contains(&DiffLine::Same {
            text: "хвост".to_string()
        }));
    }

    #[test]
    fn diff_of_identical_content_has_no_differing_lines() {
        let diff = line_diff("одна\nдве\n", "одна\nдве\n");
        assert!(diff
            .iter()
            .all(|line| matches!(line, DiffLine::Same { .. })));
        assert_eq!(diff.len(), 2);
    }

    #[test]
    fn diff_beyond_line_budget_degrades_to_showing_both_sides_whole() {
        let big: String = (0..DIFF_LINE_BUDGET + 1)
            .map(|i| format!("строка {i}\n"))
            .collect();
        let diff = line_diff(&big, "внешняя\n");
        assert!(diff
            .iter()
            .all(|line| !matches!(line, DiffLine::Same { .. })));
        assert!(diff.contains(&DiffLine::External {
            text: "внешняя".to_string()
        }));
    }
}

#[cfg(test)]
mod relocate_tests {
    use super::*;

    fn stamp(content: &str) -> FileStamp {
        FileStamp::from_content(0, content.as_bytes())
    }

    #[test]
    fn rename_prefix_carries_buffer_conflict_and_deletion_to_the_new_path() {
        let mut registry = ConflictRegistry::new();
        let edited = Path::new("A/x.typ");
        registry.set_buffer(edited, "мои правки".to_string());
        assert!(registry.observe_disk_change(edited, &stamp("было"), &stamp("внешнее")));
        let gone = Path::new("A/C/y.typ");
        registry.set_buffer(gone, "правки удалённой".to_string());
        registry.observe_disk_deletion(gone);

        let moved = registry.rename_prefix(Path::new("A"), Path::new("B/A"));

        assert!(moved >= 3);
        let new_edited = Path::new("B/A/x.typ");
        assert!(registry.has_unsaved_edits(new_edited));
        assert_eq!(registry.buffer(new_edited), Some("мои правки"));
        assert_eq!(
            registry.conflict(new_edited).map(|c| c.note_path.clone()),
            Some(new_edited.to_path_buf()),
            "конфликт держит новый путь и внутри"
        );
        assert!(registry.was_deleted_with_edits(Path::new("B/A/C/y.typ")));
        assert!(!registry.has_unsaved_edits(edited));
        assert!(!registry.was_deleted_with_edits(gone));
    }

    #[test]
    fn rename_prefix_leaves_unrelated_paths_alone() {
        let mut registry = ConflictRegistry::new();
        registry.set_buffer(Path::new("A-b/x.typ"), "чужое".to_string());
        registry.set_buffer(Path::new("A.typ"), "тоже чужое".to_string());

        assert_eq!(registry.rename_prefix(Path::new("A"), Path::new("B/A")), 0);
        assert!(registry.has_unsaved_edits(Path::new("A-b/x.typ")));
        assert!(registry.has_unsaved_edits(Path::new("A.typ")));
    }
}
