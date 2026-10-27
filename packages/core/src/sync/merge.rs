//! Трёхстороннее слияние текста (фича 011, research §11, FR-016, FR-034).
//!
//! Одно слияние на оба источника конфликтов — получение снимков с сервера и
//! конфликтные копии Syncthing: в обоих случаях есть общая версия и две
//! расходящиеся, и правила совмещения обязаны быть одинаковыми (принцип II и
//! просто здравый смысл: пользователь не должен гадать, почему одна и та же
//! пара правок совмещается в одном месте и спорит в другом).
//!
//! Правило осторожное, как у git: правки, касающиеся **соседних** строк
//! общей версии, считаются конфликтом. Абзацы, разделённые хотя бы одной
//! нетронутой строкой, совмещаются сами (SC-007).

use serde::{Deserialize, Serialize};
use similar::{capture_diff_slices, Algorithm, DiffOp};

/// Итог слияния.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum MergeResult {
    /// Правки не пересекаются — готовый текст.
    Clean { text: String },
    /// Есть участки, где стороны спорят. Порядок участков — порядок текста.
    Conflicted { hunks: Vec<Hunk> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum Hunk {
    /// Участок, по которому стороны согласны (или правила только одна).
    Same { text: String },
    /// Спорный участок: как было и как стало у каждой стороны.
    Conflict {
        base: String,
        ours: String,
        theirs: String,
    },
}

impl MergeResult {
    /// Текст с выбором одной стороны во всех спорных участках — основа для
    /// ручного совмещения в редакторе слияния.
    pub fn text_preferring(&self, ours: bool) -> String {
        match self {
            MergeResult::Clean { text } => text.clone(),
            MergeResult::Conflicted { hunks } => hunks
                .iter()
                .map(|hunk| match hunk {
                    Hunk::Same { text } => text.as_str(),
                    Hunk::Conflict {
                        ours: o, theirs: t, ..
                    } => {
                        if ours {
                            o.as_str()
                        } else {
                            t.as_str()
                        }
                    }
                })
                .collect(),
        }
    }
}

/// Похоже ли содержимое на двоичное: не UTF-8 или есть NUL в первых 8 КБ (то
/// же правило, что у git). Двоичное не совмещается — только выбор целиком.
pub fn is_binary(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(8 * 1024)];
    head.contains(&0) || std::str::from_utf8(bytes).is_err()
}

/// Изменённый участок общей версии с одной стороны.
#[derive(Debug, Clone, Copy)]
struct Change {
    /// `[start, end)` в строках общей версии.
    start: usize,
    end: usize,
    /// Сколько строк стало на месте участка.
    new_len: usize,
    ours: bool,
}

fn changes(base: &[&str], side: &[&str], ours: bool) -> Vec<Change> {
    capture_diff_slices(Algorithm::Myers, base, side)
        .into_iter()
        .filter_map(|op| match op {
            DiffOp::Equal { .. } => None,
            DiffOp::Delete {
                old_index, old_len, ..
            } => Some(Change {
                start: old_index,
                end: old_index + old_len,
                new_len: 0,
                ours,
            }),
            DiffOp::Insert {
                old_index, new_len, ..
            } => Some(Change {
                start: old_index,
                end: old_index,
                new_len,
                ours,
            }),
            DiffOp::Replace {
                old_index,
                old_len,
                new_len,
                ..
            } => Some(Change {
                start: old_index,
                end: old_index + old_len,
                new_len,
                ours,
            }),
        })
        .collect()
}

/// Сливает две версии относительно общей.
pub fn merge3(base: &str, ours: &str, theirs: &str) -> MergeResult {
    if ours == theirs {
        return MergeResult::Clean {
            text: ours.to_string(),
        };
    }
    if base == ours {
        return MergeResult::Clean {
            text: theirs.to_string(),
        };
    }
    if base == theirs {
        return MergeResult::Clean {
            text: ours.to_string(),
        };
    }

    // Участки с происхождением и прежний итог — из одного прохода (фича 040,
    // FR-006): подсветка не может разойтись со спорными участками.
    let mut hunks: Vec<Hunk> = Vec::new();
    for segment in segments(base, ours, theirs) {
        match segment {
            Segment::Agreed { text, .. } => push_text(&mut hunks, text),
            Segment::Disputed { base, ours, theirs } => {
                hunks.push(Hunk::Conflict { base, ours, theirs })
            }
        }
    }

    if hunks.iter().all(|hunk| matches!(hunk, Hunk::Same { .. })) {
        MergeResult::Clean {
            text: hunks
                .into_iter()
                .map(|hunk| match hunk {
                    Hunk::Same { text } => text,
                    Hunk::Conflict { .. } => unreachable!(),
                })
                .collect(),
        }
    } else {
        MergeResult::Conflicted { hunks }
    }
}

/// Кем тронута строка (фича 040, data-model.md). В колонке версии — `Same`,
/// `Ours`, `Theirs`, в общей колонке ещё `Both`; в результате — все пять.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LineOrigin {
    Same,
    Ours,
    Theirs,
    Both,
    Manual,
}

/// Участок слияния с происхождением: то же деление, что `Hunk`, но общий
/// участок помнит, чья правка в нём принята.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum Segment {
    /// Спора нет. `origin` — `Same` (никто не трогал), `Ours`, `Theirs` или
    /// `Both` (одинаковая правка обеих сторон).
    Agreed { text: String, origin: LineOrigin },
    /// Спорный участок — тот же, что `Hunk::Conflict`.
    Disputed {
        base: String,
        ours: String,
        theirs: String,
    },
}

/// Что показать в редакторе слияния (research §1–§2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MergeView {
    /// По строке своей версии: `Ours` — правка своей стороны.
    pub ours: Vec<LineOrigin>,
    /// По строке их версии: `Theirs` — правка их стороны.
    pub theirs: Vec<LineOrigin>,
    /// По строке общей версии; `None` — общей версии нет.
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Vec<LineOrigin>>,
    /// Участки для сборки результата; `None` — без общей версии совмещать нечего.
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<Segment>>,
}

/// Предел строк одной версии для построчной подсветки и перекраски правки
/// (040, ревью безопасности). Myers без срока — O((N+M)·D), а текст приходит от
/// клиента сервера или из чужой копии: без предела один запрос надолго занял
/// бы сервер. Заметка в 5 000 строк (SC-003) проходит с запасом.
pub const LINE_LIMIT: usize = 10_000;

fn over_limit(text: &str) -> bool {
    text.len() > LINE_LIMIT * 200 || text.split_inclusive('\n').nth(LINE_LIMIT).is_some()
}

/// Пометки строк и участки. Без общей версии своя и их сравниваются друг с
/// другом: строка, которой нет в другой версии, помечается своей стороной.
/// `None` — версия длиннее [`LINE_LIMIT`]: подсветки нет, только выбор целиком.
pub fn merge_view(base: Option<&str>, ours: &str, theirs: &str) -> Option<MergeView> {
    if over_limit(ours) || over_limit(theirs) || base.is_some_and(over_limit) {
        return None;
    }
    Some(merge_view_unbounded(base, ours, theirs))
}

fn merge_view_unbounded(base: Option<&str>, ours: &str, theirs: &str) -> MergeView {
    let o: Vec<&str> = ours.split_inclusive('\n').collect();
    let t: Vec<&str> = theirs.split_inclusive('\n').collect();
    let mut ours_marks = vec![LineOrigin::Same; o.len()];
    let mut theirs_marks = vec![LineOrigin::Same; t.len()];
    let Some(base) = base else {
        for op in capture_diff_slices(Algorithm::Myers, &t, &o) {
            let (old, new) = touched(&op);
            mark(&mut theirs_marks, old, LineOrigin::Theirs);
            mark(&mut ours_marks, new, LineOrigin::Ours);
        }
        return MergeView {
            ours: ours_marks,
            theirs: theirs_marks,
            base: None,
            segments: None,
        };
    };
    let b: Vec<&str> = base.split_inclusive('\n').collect();
    let mut base_marks = vec![LineOrigin::Same; b.len()];
    for op in capture_diff_slices(Algorithm::Myers, &b, &o) {
        let (old, new) = touched(&op);
        mark(&mut base_marks, old, LineOrigin::Ours);
        mark(&mut ours_marks, new, LineOrigin::Ours);
    }
    for op in capture_diff_slices(Algorithm::Myers, &b, &t) {
        let (old, new) = touched(&op);
        for line in &mut base_marks[old] {
            *line = match *line {
                LineOrigin::Ours | LineOrigin::Both => LineOrigin::Both,
                _ => LineOrigin::Theirs,
            };
        }
        mark(&mut theirs_marks, new, LineOrigin::Theirs);
    }
    MergeView {
        ours: ours_marks,
        theirs: theirs_marks,
        base: Some(base_marks),
        segments: Some(segments(base, ours, theirs)),
    }
}

/// Строки, затронутые операцией: удалённые/заменённые старой стороны и
/// вставленные/заменённые новой.
fn touched(op: &DiffOp) -> (std::ops::Range<usize>, std::ops::Range<usize>) {
    match *op {
        DiffOp::Equal { .. } => (0..0, 0..0),
        DiffOp::Delete {
            old_index, old_len, ..
        } => (old_index..old_index + old_len, 0..0),
        DiffOp::Insert {
            new_index, new_len, ..
        } => (0..0, new_index..new_index + new_len),
        DiffOp::Replace {
            old_index,
            old_len,
            new_index,
            new_len,
        } => (
            old_index..old_index + old_len,
            new_index..new_index + new_len,
        ),
    }
}

fn mark(marks: &mut [LineOrigin], range: std::ops::Range<usize>, origin: LineOrigin) {
    for line in &mut marks[range] {
        *line = origin;
    }
}

/// Выбор в спорном участке.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Pick {
    Ours,
    Theirs,
    /// Сначала своя часть, затем их.
    Both,
}

/// Результат слияния с происхождением каждой строки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Annotated {
    pub text: String,
    /// По строке `text`.
    pub lines: Vec<LineOrigin>,
}

/// Запрос сборки результата — одним значением, как его шлют интерфейс и
/// команда оболочки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct AnnotateRequest {
    pub segments: Vec<Segment>,
    pub picks: Vec<Pick>,
    /// Правленый вручную текст; нет — результат собирается из выбора.
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited: Option<String>,
}

impl AnnotateRequest {
    pub fn run(&self) -> Annotated {
        merge_annotate(&self.segments, &self.picks, self.edited.as_deref())
    }
}

/// Собирает результат из участков и выбора сторон (research §3). С ручной
/// правкой `edited` текст — правленый, а строки, которых в собранном нет,
/// помечаются `Manual`; остальные сохраняют пометку.
pub fn merge_annotate(segments: &[Segment], picks: &[Pick], edited: Option<&str>) -> Annotated {
    let mut text = String::new();
    let mut lines = Vec::new();
    let mut index = 0;
    for segment in segments {
        match segment {
            Segment::Agreed { text: part, origin } => {
                push_annotated(&mut text, &mut lines, part, *origin)
            }
            Segment::Disputed { ours, theirs, .. } => {
                let pick = picks.get(index).copied().unwrap_or(Pick::Ours);
                index += 1;
                match pick {
                    Pick::Ours => push_annotated(&mut text, &mut lines, ours, LineOrigin::Ours),
                    Pick::Theirs => {
                        push_annotated(&mut text, &mut lines, theirs, LineOrigin::Theirs)
                    }
                    Pick::Both => {
                        push_annotated(&mut text, &mut lines, ours, LineOrigin::Ours);
                        // Своя часть без перевода строки в конце файла склеилась
                        // бы с первой строкой их части.
                        if !ours.is_empty() && !ours.ends_with('\n') && !theirs.is_empty() {
                            text.push('\n');
                        }
                        push_annotated(&mut text, &mut lines, theirs, LineOrigin::Theirs);
                    }
                }
            }
        }
    }
    let Some(edited) = edited else {
        return Annotated { text, lines };
    };
    let old: Vec<&str> = text.split_inclusive('\n').collect();
    let new: Vec<&str> = edited.split_inclusive('\n').collect();
    // Сверх предела сравнения нет: всё правленое — «вручную» (см. LINE_LIMIT).
    if over_limit(&text) || over_limit(edited) {
        return Annotated {
            lines: vec![LineOrigin::Manual; new.len()],
            text: edited.to_string(),
        };
    }
    let mut marks = Vec::with_capacity(new.len());
    for op in capture_diff_slices(Algorithm::Myers, &old, &new) {
        match op {
            DiffOp::Equal { old_index, len, .. } => {
                marks.extend_from_slice(&lines[old_index..old_index + len])
            }
            DiffOp::Delete { .. } => {}
            DiffOp::Insert { new_len, .. } | DiffOp::Replace { new_len, .. } => {
                marks.extend(std::iter::repeat(LineOrigin::Manual).take(new_len))
            }
        }
    }
    Annotated {
        text: edited.to_string(),
        lines: marks,
    }
}

fn push_annotated(text: &mut String, lines: &mut Vec<LineOrigin>, part: &str, origin: LineOrigin) {
    if part.is_empty() {
        return;
    }
    // Продолжение незаконченной строки — та же строка, пометка у неё прежняя.
    let continues = !text.is_empty() && !text.ends_with('\n');
    let count = part.split_inclusive('\n').count() - usize::from(continues);
    text.push_str(part);
    lines.extend(std::iter::repeat(origin).take(count));
}

/// Общий проход трёхстороннего слияния: участки с происхождением.
fn segments(base: &str, ours: &str, theirs: &str) -> Vec<Segment> {
    let b: Vec<&str> = base.split_inclusive('\n').collect();
    let o: Vec<&str> = ours.split_inclusive('\n').collect();
    let t: Vec<&str> = theirs.split_inclusive('\n').collect();

    let mut all = changes(&b, &o, true);
    all.extend(changes(&b, &t, false));
    all.sort_by_key(|change| (change.start, change.end));

    let mut hunks: Vec<Segment> = Vec::new();
    let (mut pb, mut po, mut pt) = (0usize, 0usize, 0usize);
    let mut index = 0;
    while index < all.len() {
        // Группа пересекающихся или соседних правок обеих сторон.
        let group_start = all[index].start;
        let mut group_end = all[index].end;
        let mut members = vec![all[index]];
        index += 1;
        while index < all.len() && all[index].start <= group_end {
            group_end = group_end.max(all[index].end);
            members.push(all[index]);
            index += 1;
        }

        push_agreed(&mut hunks, b[pb..group_start].concat(), LineOrigin::Same);
        po += group_start - pb;
        pt += group_start - pb;

        let span = group_end - group_start;
        let side_len = |is_ours: bool| -> usize {
            let delta: isize = members
                .iter()
                .filter(|change| change.ours == is_ours)
                .map(|change| change.new_len as isize - (change.end - change.start) as isize)
                .sum();
            (span as isize + delta) as usize
        };
        let o_len = side_len(true);
        let t_len = side_len(false);
        let ours_part: String = o[po..po + o_len].concat();
        let theirs_part: String = t[pt..pt + t_len].concat();
        po += o_len;
        pt += t_len;

        let has_ours = members.iter().any(|change| change.ours);
        let has_theirs = members.iter().any(|change| !change.ours);
        if !has_theirs {
            push_agreed(&mut hunks, ours_part, LineOrigin::Ours);
        } else if !has_ours {
            push_agreed(&mut hunks, theirs_part, LineOrigin::Theirs);
        } else if ours_part == theirs_part {
            push_agreed(&mut hunks, ours_part, LineOrigin::Both);
        } else {
            hunks.push(Segment::Disputed {
                base: b[group_start..group_end].concat(),
                ours: ours_part,
                theirs: theirs_part,
            });
        }
        pb = group_end;
    }
    push_agreed(&mut hunks, b[pb..].concat(), LineOrigin::Same);
    hunks
}

/// Соседние общие участки с одним происхождением склеиваются.
fn push_agreed(segments: &mut Vec<Segment>, text: String, origin: LineOrigin) {
    if text.is_empty() {
        return;
    }
    if let Some(Segment::Agreed {
        text: previous,
        origin: last,
    }) = segments.last_mut()
    {
        if *last == origin {
            previous.push_str(&text);
            return;
        }
    }
    segments.push(Segment::Agreed { text, origin });
}

/// Соседние согласованные участки склеиваются: редактору слияния незачем
/// показывать границу там, где спора нет.
fn push_text(hunks: &mut Vec<Hunk>, text: String) {
    if text.is_empty() {
        return;
    }
    if let Some(Hunk::Same { text: previous }) = hunks.last_mut() {
        previous.push_str(&text);
    } else {
        hunks.push(Hunk::Same { text });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "= Заголовок\n\nПервый абзац.\n\nВторой абзац.\n\nТретий абзац.\n";

    #[test]
    fn edits_in_different_paragraphs_merge_cleanly() {
        let ours = BASE.replace("Первый абзац.", "Первый абзац, правка A.");
        let theirs = BASE.replace("Третий абзац.", "Третий абзац, правка B.");
        assert_eq!(
            merge3(BASE, &ours, &theirs),
            MergeResult::Clean {
                text: "= Заголовок\n\nПервый абзац, правка A.\n\nВторой абзац.\n\nТретий абзац, правка B.\n"
                    .to_string()
            }
        );
    }

    #[test]
    fn edits_of_the_same_line_conflict() {
        let ours = BASE.replace("Второй абзац.", "Второй — A.");
        let theirs = BASE.replace("Второй абзац.", "Второй — B.");
        let MergeResult::Conflicted { hunks } = merge3(BASE, &ours, &theirs) else {
            panic!("ожидался конфликт");
        };
        assert!(hunks.contains(&Hunk::Conflict {
            base: "Второй абзац.\n".into(),
            ours: "Второй — A.\n".into(),
            theirs: "Второй — B.\n".into(),
        }));
        let merged = merge3(BASE, &ours, &theirs);
        assert_eq!(merged.text_preferring(true), ours);
        assert_eq!(merged.text_preferring(false), theirs);
    }

    #[test]
    fn identical_edits_are_not_a_conflict() {
        let both = BASE.replace("Второй абзац.", "Одно и то же.");
        assert_eq!(
            merge3(BASE, &both, &both),
            MergeResult::Clean { text: both.clone() }
        );
    }

    #[test]
    fn additions_at_both_ends_merge() {
        let ours = format!("Начало.\n{BASE}");
        let theirs = format!("{BASE}Конец.\n");
        assert_eq!(
            merge3(BASE, &ours, &theirs),
            MergeResult::Clean {
                text: format!("Начало.\n{BASE}Конец.\n")
            }
        );
    }

    #[test]
    fn deletion_against_edit_conflicts() {
        let ours = BASE.replace("Второй абзац.\n", "");
        let theirs = BASE.replace("Второй абзац.", "Второй, исправлен.");
        assert!(matches!(
            merge3(BASE, &ours, &theirs),
            MergeResult::Conflicted { .. }
        ));
    }

    #[test]
    fn binary_detection() {
        assert!(is_binary(&[0x89, b'P', b'N', b'G', 0, 1]));
        assert!(!is_binary("текст".as_bytes()));
    }
}
