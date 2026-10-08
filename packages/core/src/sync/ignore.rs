//! Правила исключения истории и Syncthing (фича 011, research §5, §6).
//!
//! Оба файла принадлежат пользователю. Приложение ведёт в них **свой блок**
//! между маркерами-комментариями и не трогает ни одной строки вне него: так
//! повторное применение не плодит дублей (FR-027), а правила пользователя
//! переживают любое количество исправлений.
//!
//! Место блока различается намеренно:
//! - в `.stignore` выигрывает **первое** совпавшее правило, поэтому блок стоит в
//!   начале — иначе `!…` пользователя выше по файлу отменило бы исключение
//!   `.git`, и Syncthing разнёс бы историю по устройствам;
//! - в `.gitignore` выигрывает **последнее**, поэтому блок — в конце.

use serde::{Deserialize, Serialize};

/// Какой файл правил.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum IgnoreFile {
    /// Правила Syncthing, `.stignore` в корне vault. Syncthing его **не
    /// доставляет** между устройствами — у каждого свой (research §5).
    StIgnore,
    /// Правила истории, `.gitignore` в корне vault.
    GitIgnore,
}

impl IgnoreFile {
    pub fn path(self) -> &'static str {
        match self {
            IgnoreFile::StIgnore => ".stignore",
            IgnoreFile::GitIgnore => ".gitignore",
        }
    }

    fn comment(self) -> &'static str {
        match self {
            IgnoreFile::StIgnore => "//",
            IgnoreFile::GitIgnore => "#",
        }
    }

    fn begin_marker(self) -> String {
        format!("{} typsidian:begin", self.comment())
    }

    fn end_marker(self) -> String {
        format!("{} typsidian:end", self.comment())
    }

    /// Правила, которые приложение считает обязательными для этого файла.
    pub fn required_rules(self) -> Vec<String> {
        match self {
            // Служебный каталог истории и производные кэши (FR-026). Настройки
            // vault и локальные шаблоны доставляются: они общие для устройств.
            IgnoreFile::StIgnore => {
                let mut rules = vec!["/.git".to_string()];
                rules.extend(derived_rules());
                rules
            }
            // Кэши, служебные файлы Syncthing и его конфликтные копии (FR-003).
            // Конфликтная копия в истории — это мусор, который потом
            // приходится вычищать из каждого снимка.
            IgnoreFile::GitIgnore => {
                let mut rules = derived_rules();
                rules.extend(
                    [
                        "*.sync-conflict-*",
                        "/.stfolder/",
                        "/.stversions/",
                        "/.stignore",
                        ".syncthing.*.tmp",
                        "~syncthing~*.tmp",
                    ]
                    .map(str::to_string),
                );
                rules
            }
        }
    }
}

/// Производные пути служебного места в виде правил: общая часть обоих файлов.
fn derived_rules() -> Vec<String> {
    crate::vault::service_dir::derived_paths()
        .into_iter()
        .map(|path| format!("/{path}"))
        .collect()
}

/// Чего не хватает в файле правил и что приложение предлагает дописать.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct IgnoreReport {
    pub file: IgnoreFile,
    /// Обязательные правила, которых нет ни в блоке приложения, ни среди строк
    /// пользователя.
    pub missing: Vec<String>,
    /// Точный текст блока, который появится в файле (FR-027: «показав точный
    /// текст»).
    pub proposed_block: String,
    /// Сколько строк пользователя в файле — показывается, чтобы было видно, что
    /// они сохранятся.
    pub user_lines: u32,
}

impl IgnoreReport {
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }
}

/// Отчёт о файле правил. `current` — его содержимое, `None` — файла нет.
pub fn report(file: IgnoreFile, current: Option<&str>) -> IgnoreReport {
    let parts = split(file, current.unwrap_or(""));
    let present: Vec<&str> = parts
        .block
        .iter()
        .chain(parts.before.iter())
        .chain(parts.after.iter())
        .map(|line| line.trim())
        .collect();
    let missing = file
        .required_rules()
        .into_iter()
        .filter(|rule| !present.iter().any(|line| covers(line, rule)))
        .collect();
    let user_lines = parts
        .before
        .iter()
        .chain(parts.after.iter())
        .filter(|line| !line.trim().is_empty())
        .count() as u32;
    IgnoreReport {
        file,
        missing,
        proposed_block: render_block(file),
        user_lines,
    }
}

/// Содержимое файла после применения блока приложения.
///
/// Блок заменяется целиком, если он уже есть; строки вне блока сохраняются
/// байт в байт. Применение к собственному результату ничего не меняет.
pub fn apply(file: IgnoreFile, current: Option<&str>) -> String {
    let text = current.unwrap_or("");
    let parts = split(file, text);
    let block = render_block(file);
    let user_before = join(&parts.before);
    let user_after = join(&parts.after);
    match file {
        IgnoreFile::StIgnore => {
            let mut out = block;
            let rest = [user_before, user_after]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if !rest.is_empty() {
                out.push('\n');
                out.push_str(&rest);
                out.push('\n');
            }
            out
        }
        IgnoreFile::GitIgnore => {
            let rest = [user_before, user_after]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if rest.is_empty() {
                block
            } else {
                format!("{rest}\n\n{block}")
            }
        }
    }
}

fn render_block(file: IgnoreFile) -> String {
    let mut lines = vec![
        file.begin_marker(),
        format!(
            "{} Правила Typsidian: история и кэши не синхронизируются. Строки вне блока не меняются.",
            file.comment()
        ),
    ];
    lines.extend(file.required_rules());
    lines.push(file.end_marker());
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

struct Parts<'a> {
    before: Vec<&'a str>,
    block: Vec<&'a str>,
    after: Vec<&'a str>,
}

/// Делит файл на строки до блока, сам блок и строки после.
///
/// Незакрытый блок (маркер начала без конца — файл правили руками или он
/// пришёл таким из репозитория) тянется только по строкам, которые писало
/// приложение: его правила — наши, и вторая копия рядом была бы дублем, но
/// строки пользователя ниже блоком не становятся и при замене не пропадают
/// (T068).
fn split(file: IgnoreFile, text: &str) -> Parts<'_> {
    let begin = file.begin_marker();
    let end = file.end_marker();
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|line| line.trim() == begin) else {
        return Parts {
            before: trim_blank_edges(lines),
            block: Vec::new(),
            after: Vec::new(),
        };
    };
    let Some(stop) = lines[start..]
        .iter()
        .position(|line| line.trim() == end)
        .map(|offset| start + offset)
    else {
        let own = render_block(file);
        let mut stop = start + 1;
        while lines
            .get(stop)
            .map(|line| line.trim())
            .is_some_and(|line| line.is_empty() || own.lines().any(|ours| ours == line))
        {
            stop += 1;
        }
        return Parts {
            before: trim_blank_edges(lines[..start].to_vec()),
            block: lines[start + 1..stop].to_vec(),
            after: trim_blank_edges(lines[stop..].to_vec()),
        };
    };
    Parts {
        before: trim_blank_edges(lines[..start].to_vec()),
        block: lines[start + 1..stop].to_vec(),
        after: trim_blank_edges(lines[(stop + 1).min(lines.len())..].to_vec()),
    }
}

fn trim_blank_edges(mut lines: Vec<&str>) -> Vec<&str> {
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines
}

fn join(lines: &[&str]) -> String {
    lines.join("\n")
}

/// Покрывает ли строка файла обязательное правило. Сравнение с точностью до
/// ведущего `/` и завершающего `/`: `.git`, `/.git` и `/.git/` исключают одно и
/// то же место в корне vault.
fn covers(line: &str, rule: &str) -> bool {
    fn norm(value: &str) -> &str {
        value.trim_start_matches('/').trim_end_matches('/')
    }
    !line.starts_with('!') && norm(line) == norm(rule)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_file_misses_every_rule() {
        let report = report(IgnoreFile::StIgnore, None);
        assert_eq!(report.missing, IgnoreFile::StIgnore.required_rules());
        assert_eq!(report.user_lines, 0);
    }

    #[test]
    fn applying_twice_is_the_same_as_once() {
        let once = apply(IgnoreFile::StIgnore, Some("// мои\n*.tmp\n"));
        let twice = apply(IgnoreFile::StIgnore, Some(&once));
        assert_eq!(once, twice);
        assert!(report(IgnoreFile::StIgnore, Some(&once)).is_complete());
    }

    #[test]
    fn user_lines_survive_and_stignore_block_goes_first() {
        let applied = apply(IgnoreFile::StIgnore, Some("!/keep.typ\n*.tmp"));
        assert!(applied.starts_with("// typsidian:begin"));
        assert!(applied.contains("!/keep.typ\n*.tmp"));
    }

    #[test]
    fn gitignore_block_goes_last_and_keeps_user_rules() {
        let applied = apply(IgnoreFile::GitIgnore, Some("result*\n*.pdf\n"));
        assert!(applied.starts_with("result*\n*.pdf\n"));
        assert!(applied.trim_end().ends_with("# typsidian:end"));
    }

    #[test]
    fn a_user_rule_counts_as_present() {
        let report = report(IgnoreFile::StIgnore, Some(".git/\n"));
        assert!(!report.missing.contains(&"/.git".to_string()));
        assert_eq!(report.user_lines, 1);
    }

    #[test]
    fn an_unclosed_block_neither_panics_nor_swallows_user_lines() {
        for file in [IgnoreFile::StIgnore, IgnoreFile::GitIgnore] {
            // Маркер начала последней строкой — раньше здесь была паника.
            let lone = format!("мои-правила\n{}", file.begin_marker());
            assert!(apply(file, Some(&lone)).contains("мои-правила"));

            // Строки пользователя после незакрытого блока остаются.
            let rules = file.required_rules();
            let torn = format!("{}\n{}\nмоё-ниже\nещё-моё\n", file.begin_marker(), rules[0]);
            let fixed = apply(file, Some(&torn));
            assert!(
                fixed.contains("моё-ниже") && fixed.contains("ещё-моё"),
                "{fixed}"
            );
            assert_eq!(fixed.matches(&file.begin_marker()).count(), 1);
            assert_eq!(fixed.matches(rules[0].as_str()).count(), 1);
            assert_eq!(apply(file, Some(&fixed)), fixed);
        }
    }

    #[test]
    fn a_negated_rule_does_not_count() {
        let report = report(IgnoreFile::StIgnore, Some("!.git\n"));
        assert!(report.missing.contains(&"/.git".to_string()));
    }
}
