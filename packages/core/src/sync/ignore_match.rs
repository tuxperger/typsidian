//! Исключает ли файл правил папку (фича 016, FR-020, research.md §12).
//!
//! Вставка предупреждает, если папка вложений не попадёт в историю или не
//! будет доставлена. Готового сопоставления путей в ядре нет: `sync::ignore`
//! ведёт блок обязательных правил и сверяет их дословно, а `git2` отвечает
//! только на native и только при существующем репозитории. Здесь — одна
//! реализация для обоих таргетов (принцип II).
//!
//! Поддерживается: комментарии, пустые строки, `!`, якорь `/`, завершающий
//! `/`, `*`, `?`, `[…]`, `**`; для `.stignore` — `(?i)`, `(?d)` и `{a,b}`. В
//! `.gitignore` выигрывает последнее совпавшее правило, в `.stignore` —
//! первое. Папка исключена, если исключена она сама или любая папка над ней.
//!
//! Не поддерживается (ограничение): `#include` в `.stignore`, вложенные
//! `.gitignore`, глобальные правила git, правила каталогов выше vault.

use regex::Regex;

use super::ignore::IgnoreFile;

/// Исключает ли текст правил `file` папку `path` (от корня vault, через `/`).
pub fn excludes(file: IgnoreFile, text: &str, path: &str) -> bool {
    let rules: Vec<Rule> = text
        .lines()
        .filter_map(|line| Rule::parse(file, line))
        .collect();
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    // Сверху вниз: исключённая папка исключает всё, что под ней, и отмена
    // правилом ниже её содержимое не возвращает (так ведёт себя git).
    (1..=parts.len()).any(|depth| decide(file, &rules, &parts[..depth].join("/")))
}

/// Решение для одного пути по порядку правил файла.
fn decide(file: IgnoreFile, rules: &[Rule], path: &str) -> bool {
    let mut matching = rules.iter().filter(|rule| rule.pattern.is_match(path));
    match file {
        IgnoreFile::GitIgnore => matching.last().is_some_and(|rule| !rule.negated),
        IgnoreFile::StIgnore => matching.next().is_some_and(|rule| !rule.negated),
    }
}

struct Rule {
    negated: bool,
    pattern: Regex,
}

impl Rule {
    fn parse(file: IgnoreFile, line: &str) -> Option<Rule> {
        let mut text = line.trim_end_matches(['\r', ' ']);
        if file == IgnoreFile::StIgnore {
            text = text.trim_start();
        }
        if text.is_empty() || text.starts_with('#') {
            return None;
        }
        if file == IgnoreFile::StIgnore && text.starts_with("//") {
            return None;
        }
        let mut negated = false;
        let mut case_insensitive = false;
        if file == IgnoreFile::StIgnore {
            // Префиксы Syncthing — в любом порядке.
            loop {
                if let Some(rest) = text.strip_prefix('!') {
                    negated = true;
                    text = rest;
                } else if let Some(rest) = text.strip_prefix("(?i)") {
                    case_insensitive = true;
                    text = rest;
                } else if let Some(rest) = text.strip_prefix("(?d)") {
                    text = rest;
                } else {
                    break;
                }
            }
        } else if let Some(rest) = text.strip_prefix('!') {
            negated = true;
            text = rest;
        } else if let Some(rest) = text.strip_prefix('\\') {
            // `\#` и `\!` — буквальные символы в начале имени.
            text = rest;
        }

        // Завершающий `/` — «только папка»; сопоставляются здесь только папки.
        let text = text.strip_suffix('/').unwrap_or(text);
        if text.is_empty() {
            return None;
        }
        // Якорь: ведущий `/`, а у `.gitignore` и `/` в середине шаблона.
        let (anchored, body) = match text.strip_prefix('/') {
            Some(rest) => (true, rest),
            None => (file == IgnoreFile::GitIgnore && text.contains('/'), text),
        };
        let translated = translate(body, file == IgnoreFile::StIgnore)?;
        let prefix = if anchored { "^" } else { "^(?:.*/)?" };
        let flags = if case_insensitive { "(?i)" } else { "" };
        let pattern = Regex::new(&format!("{flags}{prefix}{translated}$")).ok()?;
        Some(Rule { negated, pattern })
    }
}

/// Шаблон → тело регулярного выражения. `None` — шаблон не разбирается (тогда
/// правило пропускается, а не роняет проверку).
fn translate(pattern: &str, alternation: bool) -> Option<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut braces = 0usize;
    while i < chars.len() {
        let character = chars[i];
        match character {
            '*' if chars.get(i + 1) == Some(&'*') => {
                let at_start = i == 0 || chars[i - 1] == '/';
                let then_slash = chars.get(i + 2) == Some(&'/');
                let at_end = i + 2 == chars.len();
                if at_start && then_slash {
                    // `**/` — любое число папок, в том числе ни одной.
                    out.push_str("(?:.*/)?");
                    i += 3;
                } else if at_start && at_end {
                    out.push_str(".*");
                    i += 2;
                } else {
                    out.push_str("[^/]*");
                    i += 2;
                }
                continue;
            }
            '*' => out.push_str("[^/]*"),
            '?' => out.push_str("[^/]"),
            '[' => {
                let end = chars[i + 1..].iter().position(|&c| c == ']')? + i + 1;
                let mut class: String = chars[i + 1..end].iter().collect();
                if let Some(rest) = class.strip_prefix('!') {
                    class = format!("^{rest}");
                }
                out.push('[');
                out.push_str(&class.replace('\\', "\\\\"));
                out.push(']');
                i = end + 1;
                continue;
            }
            '{' if alternation => {
                braces += 1;
                out.push_str("(?:");
            }
            '}' if alternation && braces > 0 => {
                braces -= 1;
                out.push(')');
            }
            ',' if alternation && braces > 0 => out.push('|'),
            '\\' if i + 1 < chars.len() => {
                i += 1;
                out.push_str(&regex::escape(&chars[i].to_string()));
            }
            other => out.push_str(&regex::escape(&other.to_string())),
        }
        i += 1;
    }
    (braces == 0).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIT: IgnoreFile = IgnoreFile::GitIgnore;
    const ST: IgnoreFile = IgnoreFile::StIgnore;

    #[test]
    fn folder_rule_and_anchor() {
        assert!(excludes(GIT, "attachments/\n", "Глава/attachments"));
        assert!(excludes(GIT, "/attachments\n", "attachments"));
        assert!(!excludes(GIT, "/attachments\n", "Глава/attachments"));
        assert!(excludes(GIT, "Глава/attachments\n", "Глава/attachments"));
        assert!(!excludes(
            GIT,
            "Глава/attachments\n",
            "Другое/Глава/attachments"
        ));
    }

    #[test]
    fn double_star_and_wildcards() {
        assert!(excludes(GIT, "**/att*\n", "a/b/attachments"));
        assert!(excludes(GIT, "a/**/files\n", "a/x/y/files"));
        assert!(excludes(GIT, "a/**/files\n", "a/files"));
        assert!(excludes(GIT, "fil?s\n", "files"));
        assert!(excludes(GIT, "[fg]iles\n", "giles"));
        assert!(!excludes(GIT, "[!fg]iles\n", "files"));
    }

    #[test]
    fn last_rule_wins_in_gitignore_first_in_stignore() {
        assert!(!excludes(GIT, "files\n!files\n", "files"));
        assert!(excludes(GIT, "!files\nfiles\n", "files"));
        assert!(!excludes(ST, "!files\nfiles\n", "files"));
        assert!(excludes(ST, "files\n!files\n", "files"));
    }

    #[test]
    fn a_folder_above_excludes_everything_below() {
        assert!(excludes(GIT, "Архив/\n", "Архив/Глава/attachments"));
        // Отмена ниже исключённой папки содержимое не возвращает.
        assert!(excludes(
            GIT,
            "Архив/\n!Архив/Глава\n",
            "Архив/Глава/attachments"
        ));
    }

    #[test]
    fn stignore_prefixes_and_alternation() {
        assert!(excludes(ST, "(?i)ATTACHMENTS\n", "attachments"));
        assert!(!excludes(ST, "ATTACHMENTS\n", "attachments"));
        assert!(excludes(ST, "(?d)files\n", "files"));
        assert!(excludes(ST, "{files,attachments}\n", "x/attachments"));
        assert!(excludes(ST, "/files\n", "files"));
        assert!(!excludes(ST, "/files\n", "x/files"));
    }

    #[test]
    fn comments_and_blank_lines_are_not_rules() {
        assert!(!excludes(GIT, "# attachments\n\n", "attachments"));
        assert!(!excludes(
            ST,
            "// attachments\n#include other\n",
            "attachments"
        ));
        assert!(excludes(GIT, "\\#files\n", "#files"));
    }

    #[test]
    fn file_rules_do_not_exclude_a_folder_they_do_not_name() {
        assert!(!excludes(GIT, "*.png\n", "attachments"));
    }
}
