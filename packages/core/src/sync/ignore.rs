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
    let text = current.unwrap_or("");
    let parts = split(file, text);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let missing = file
        .required_rules()
        .into_iter()
        .filter(|rule| !in_effect(file, &lines, rule))
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
fn norm(value: &str) -> &str {
    value.trim_start_matches('/').trim_end_matches('/')
}

fn covers(line: &str, rule: &str) -> bool {
    !line.starts_with('!') && norm(line) == norm(rule)
}

/// Строка — дословная отмена правила: то же правило со знаком `!`.
fn negates(line: &str, rule: &str) -> bool {
    line.strip_prefix('!')
        .is_some_and(|rest| norm(rest.trim()) == norm(rule))
}

/// Действует ли обязательное правило в файле (012 §FR-013).
///
/// Правило должно быть в файле и не быть отменено дословно. Какая из двух
/// строк сильнее, решает формат — тем же порядком, которым он выбирает место
/// блока (см. заголовок модуля): в `.gitignore` выигрывает последняя, в
/// `.stignore` — первая. Отмены шаблоном здесь не рассчитываются.
fn in_effect(file: IgnoreFile, lines: &[&str], rule: &str) -> bool {
    let mut decisive = lines
        .iter()
        .filter(|line| covers(line, rule) || negates(line, rule));
    let winner = match file {
        IgnoreFile::GitIgnore => decisive.next_back(),
        IgnoreFile::StIgnore => decisive.next(),
    };
    winner.is_some_and(|line| covers(line, rule))
}

// ── Редактор правил (фича 012) ─────────────────────────────────────────────

/// Вид окончаний строк файла правил (012 §FR-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    /// `CrLf` — только когда каждый перевод строки в файле — `\r\n`. Файл со
    /// смешанными окончаниями считается `Lf` (012 research §5).
    pub fn detect(text: &str) -> Self {
        let total = text.matches('\n').count();
        if total > 0 && text.matches("\r\n").count() == total {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        }
    }

    /// Текст файла в виде, который правит редактор: только `\n`.
    pub fn to_editor(text: &str) -> String {
        text.replace("\r\n", "\n")
    }

    /// Текст редактора с окончаниями файла.
    pub fn to_file(self, text: &str) -> String {
        let unix = Self::to_editor(text);
        match self {
            LineEnding::Lf => unix,
            LineEnding::CrLf => unix.replace('\n', "\r\n"),
        }
    }
}

/// Отметка версии файла на диске: по ней сохранение замечает чужую правку
/// (012 §FR-014). Хэш содержимого, а не время: файл, переписанный тем же
/// содержимым, расхождением не считается.
pub fn version_mark(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Строки текста от `start` (включительно) до `end` (не включая), с нуля.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

/// Можно ли править файл правил и почему нет (012 §FR-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Editability {
    Editable,
    /// Содержимое — не текст в UTF-8.
    NotText,
    /// Больше предела размера заметки.
    TooLarge,
    /// Под этим именем — символическая ссылка либо то, что не читается как
    /// файл.
    NotAFile,
    /// Vault открыт без права записи.
    ReadOnly,
}

/// Проверка текста редактора (012 §FR-010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct IgnoreCheck {
    /// Обязательные правила, которых нет или которые дословно отменены, — то
    /// же, что [`IgnoreReport::missing`] для этого текста.
    pub missing: Vec<String>,
    /// Строки блока приложения вместе с маркерами — для подсветки.
    pub block: Option<LineRange>,
    pub proposed_block: String,
}

/// Файл правил, каким его видит редактор (012 data-model).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct IgnoreDocument {
    pub file: IgnoreFile,
    pub exists: bool,
    /// Содержимое с окончаниями `\n`. Для отсутствующего файла — предложенный
    /// блок приложения, для нередактируемого — пусто.
    pub text: String,
    pub line_ending: LineEnding,
    /// Отметка версии на диске; `None` — файла нет.
    pub base: Option<String>,
    pub editable: Editability,
    /// Действуют ли правила сейчас: есть история (`GitIgnore`) или папка
    /// доставляется Syncthing (`StIgnore`).
    pub in_effect: bool,
    /// Vault внутри чужого репозитория: действуют и правила каталогов выше.
    pub foreign_parent: bool,
    pub check: IgnoreCheck,
}

/// Запрос сохранения (012 data-model).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct IgnoreSaveRequest {
    pub file: IgnoreFile,
    /// Текст редактора, окончания `\n`.
    pub text: String,
    pub line_ending: LineEnding,
    /// Отметка версии, с которой начата правка.
    pub base: Option<String>,
    /// Пользователь подтвердил сохранение без обязательных правил.
    pub confirm_incomplete: bool,
    /// Пользователь выбрал запись поверх версии, изменившейся на диске.
    pub overwrite: bool,
}

/// Почему сохранение не выполнено (012 §FR-016, §FR-017).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum IgnoreRefusal {
    NotText,
    TooLarge,
    NotAFile,
    ReadOnly,
    /// У истории незавершённая операция — правила истории сейчас не пишутся.
    HistoryBusy,
}

impl IgnoreRefusal {
    pub fn from_editability(editable: Editability) -> Option<Self> {
        match editable {
            Editability::Editable => None,
            Editability::NotText => Some(IgnoreRefusal::NotText),
            Editability::TooLarge => Some(IgnoreRefusal::TooLarge),
            Editability::NotAFile => Some(IgnoreRefusal::NotAFile),
            Editability::ReadOnly => Some(IgnoreRefusal::ReadOnly),
        }
    }
}

/// Исход сохранения. Файл записан только при `Saved`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum IgnoreSaveOutcome {
    Saved {
        document: IgnoreDocument,
    },
    /// На диске не та версия, с которой начата правка (в том числе файл
    /// удалён: `disk.exists == false`).
    Diverged {
        disk: IgnoreDocument,
        diff: Vec<crate::vault::conflict::DiffLine>,
    },
    /// Не хватает обязательных правил, подтверждения нет.
    NeedsConfirmation {
        missing: Vec<String>,
    },
    Refused {
        reason: IgnoreRefusal,
    },
}

/// Проверка текста редактора. Чистая: к диску не обращается.
pub fn check(file: IgnoreFile, text: &str) -> IgnoreCheck {
    let report = report(file, Some(text));
    IgnoreCheck {
        missing: report.missing,
        block: block_range(file, text),
        proposed_block: report.proposed_block,
    }
}

/// Текст редактора с возвращённым блоком приложения (012 §FR-011): то же, что
/// [`apply`], — строки вне блока не меняются, повтор ничего не добавляет.
pub fn restore_block(file: IgnoreFile, text: &str) -> String {
    apply(file, Some(text))
}

/// Где в тексте блок приложения, считая маркеры. Границы — те же, что видит
/// [`split`], включая незакрытый блок.
fn block_range(file: IgnoreFile, text: &str) -> Option<LineRange> {
    let begin = file.begin_marker();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|line| line.trim() == begin)?;
    let inner = split(file, text).block.len();
    let closed = lines
        .get(start + 1 + inner)
        .is_some_and(|line| line.trim() == file.end_marker());
    let end = start + 1 + inner + usize::from(closed);
    Some(LineRange {
        start: start as u32,
        end: end as u32,
    })
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

    // ── Редактор правил (фича 012) ─────────────────────────────────────────

    #[test]
    fn check_agrees_with_report() {
        for file in [IgnoreFile::StIgnore, IgnoreFile::GitIgnore] {
            for text in ["", "*.tmp\n", &apply(file, Some("*.tmp\n"))] {
                assert_eq!(check(file, text).missing, report(file, Some(text)).missing);
            }
        }
    }

    #[test]
    fn gitignore_negation_below_the_rule_cancels_it() {
        let full = apply(IgnoreFile::GitIgnore, None);
        assert!(check(IgnoreFile::GitIgnore, &full).missing.is_empty());
        let cancelled = format!("{full}!*.sync-conflict-*\n");
        assert_eq!(
            check(IgnoreFile::GitIgnore, &cancelled).missing,
            vec!["*.sync-conflict-*".to_string()]
        );
        // Отмена выше правила в `.gitignore` бессильна: выигрывает последняя.
        let harmless = format!("!*.sync-conflict-*\n{full}");
        assert!(check(IgnoreFile::GitIgnore, &harmless).missing.is_empty());
    }

    #[test]
    fn stignore_negation_above_the_rule_cancels_it() {
        let full = apply(IgnoreFile::StIgnore, None);
        // В `.stignore` выигрывает первая строка: отмена ниже блока бессильна.
        let harmless = format!("{full}!/.git\n");
        assert!(check(IgnoreFile::StIgnore, &harmless).missing.is_empty());
        let cancelled = format!("!/.git\n{full}");
        assert_eq!(
            check(IgnoreFile::StIgnore, &cancelled).missing,
            vec!["/.git".to_string()]
        );
    }

    #[test]
    fn block_range_covers_markers_and_rules() {
        let text = format!("*.tmp\n\n{}", apply(IgnoreFile::GitIgnore, None));
        let range = check(IgnoreFile::GitIgnore, &text).block.unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[range.start as usize], "# typsidian:begin");
        assert_eq!(lines[range.end as usize - 1], "# typsidian:end");
        assert_eq!(range.end as usize, lines.len());

        assert_eq!(check(IgnoreFile::GitIgnore, "*.tmp\n").block, None);
    }

    #[test]
    fn block_range_of_an_unclosed_block_stops_before_user_lines() {
        let text = "# typsidian:begin\n*.sync-conflict-*\nмоё-правило\n";
        let range = check(IgnoreFile::GitIgnore, text).block.unwrap();
        assert_eq!((range.start, range.end), (0, 2));
    }

    #[test]
    fn restoring_the_block_keeps_user_lines_and_is_idempotent() {
        let user = "# моё\n*.pdf\nчерновики/\n";
        let broken = format!("{user}\n# typsidian:begin\n# typsidian:end\n");
        let once = restore_block(IgnoreFile::GitIgnore, &broken);
        assert!(once.starts_with(user));
        assert!(check(IgnoreFile::GitIgnore, &once).missing.is_empty());
        assert_eq!(restore_block(IgnoreFile::GitIgnore, &once), once);
    }

    #[test]
    fn line_endings_round_trip() {
        assert_eq!(LineEnding::detect("a\r\nb\r\n"), LineEnding::CrLf);
        assert_eq!(LineEnding::detect("a\nb\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("a\r\nb\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("без перевода"), LineEnding::Lf);

        let file = "a\r\nb";
        let edited = LineEnding::to_editor(file);
        assert_eq!(edited, "a\nb");
        assert_eq!(LineEnding::CrLf.to_file(&edited), file);
        assert_eq!(LineEnding::Lf.to_file("a\nb\n"), "a\nb\n");
    }

    #[test]
    fn version_mark_follows_content() {
        assert_eq!(version_mark(b"a\n"), version_mark(b"a\n"));
        assert_ne!(version_mark(b"a\n"), version_mark(b"a\r\n"));
    }
}
