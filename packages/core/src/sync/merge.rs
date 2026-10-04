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

    let b: Vec<&str> = base.split_inclusive('\n').collect();
    let o: Vec<&str> = ours.split_inclusive('\n').collect();
    let t: Vec<&str> = theirs.split_inclusive('\n').collect();

    let mut all = changes(&b, &o, true);
    all.extend(changes(&b, &t, false));
    all.sort_by_key(|change| (change.start, change.end));

    let mut hunks: Vec<Hunk> = Vec::new();
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

        push_same(&mut hunks, &b[pb..group_start]);
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
        if !has_theirs || ours_part == theirs_part {
            push_text(&mut hunks, ours_part);
        } else if !has_ours {
            push_text(&mut hunks, theirs_part);
        } else {
            hunks.push(Hunk::Conflict {
                base: b[group_start..group_end].concat(),
                ours: ours_part,
                theirs: theirs_part,
            });
        }
        pb = group_end;
    }
    push_same(&mut hunks, &b[pb..]);

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

fn push_same(hunks: &mut Vec<Hunk>, lines: &[&str]) {
    push_text(hunks, lines.concat());
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
