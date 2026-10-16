//! Расширенный поиск по vault (макет «Typsidian Утилиты» § Поиск).
//!
//! Поиск живёт в ядре, а не в интерфейсе, по той же причине, что и всё
//! остальное: правила совпадения — часть смысла приложения, и они обязаны быть
//! одинаковыми на обоих клиентах (принцип I, принцип II). Интерфейс получает
//! готовые группы, готовые счётчики и **готовые куски строк** с пометкой, что
//! в них подсвечивать.
//!
//! # Почему куски строк, а не смещения
//!
//! Смещение в строке — величина, зависящая от кодировки: Rust считает байты
//! или скалярные значения, JS режет строки по UTF-16. Отдать индексы значило бы
//! завести на границе ещё одно правило, которое некому проверить. Поэтому ядро
//! возвращает строку уже разрезанной на куски, а UI просто оборачивает
//! помеченные.
//!
//! # Условия по свойствам (фича 020)
//!
//! Теги и поля заметок появились с фичей 020: условия по ним лежат в
//! [`SearchQuery::properties`], отбирает их тот же код, что и блоки выборки
//! ([`crate::properties::query`]), по индексу — без чтения файлов.
//! [`parse_conditions`] превращает набранное `тег:проект статус:черновик` в
//! эти условия: интерфейс не разбирает синтаксис запроса сам (принцип I).

use serde::{Deserialize, Serialize};

use crate::properties::query::{matches_all, CompareOp, Condition, DateRange, Ymd};
use crate::properties::{now_ms, valid_date, PropertyValue};
use crate::vault::backend::VaultBackend;
use crate::vault::FileKind;

/// Сколько заметок попадает в ответ. Дальше — `truncated`.
///
/// Предел существует ради интерфейса, а не ради скорости: список на десять
/// тысяч заметок никто не читает, а собрать его — значит удержать в памяти
/// столько же кусков текста. Пользователь уточняет запрос, и это правильное
/// поведение поиска.
pub const MAX_NOTES: usize = 200;

/// Сколько совпадений показывается внутри одной заметки.
///
/// Счётчик при этом остаётся полным: «4 совпадения» в шапке заметки — это все
/// найденные, а не показанные (макет § Поиск).
pub const MAX_HITS_PER_NOTE: usize = 3;

/// Как сложить строку запроса и фильтры (переключатель «И / ИЛИ» в макете).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum FilterMode {
    /// Заметка обязана удовлетворять и строке, и каждому фильтру.
    #[default]
    And,
    /// Достаточно одного условия.
    Or,
}

/// Что ищем (data-model макета § Поиск).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SearchQuery {
    /// Строка поиска. Пустая — условие не задано.
    pub text: String,
    /// Трактовать строку как регулярное выражение (чип «regex»).
    pub regex: bool,
    /// Различать регистр (чип «Aa»).
    pub case_sensitive: bool,
    /// Путь заметки, на которую найденная обязана ссылаться (чип «ссылается
    /// на …»). Считается по тем же ссылкам, что и обратные ссылки фичи 001.
    pub links_to: Option<String>,
    /// Условия по свойствам заметок (фича 020): теги, поля, папка, даты. Все
    /// вместе — одно условие в смысле [`FilterMode`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub properties: Vec<Condition>,
    pub mode: FilterMode,
}

impl SearchQuery {
    /// Задано ли хоть одно условие.
    ///
    /// Пустой запрос не означает «вся папка»: показать все заметки — это
    /// дерево слева, а не поиск.
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.links_to.is_none() && self.properties.is_empty()
    }
}

/// Кусок строки: совпавший или обычный.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LineSegment {
    pub text: String,
    pub highlighted: bool,
}

/// Строка заметки в выдаче: сама совпавшая либо соседняя, для контекста.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct HitLine {
    /// Номер строки в файле, считая с единицы.
    pub number: usize,
    pub segments: Vec<LineSegment>,
    /// `false` — строка показана только ради контекста.
    pub is_match: bool,
}

/// Найденная заметка.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SearchHit {
    pub path: String,
    pub name: String,
    /// Папка заметки относительно корня vault; пустая строка — корень.
    pub folder: String,
    /// Всего совпадений в заметке — не число показанных строк.
    pub matches: usize,
    /// Номер первой совпавшей строки («стр. 21» в макете); `0`, если заметка
    /// попала в выдачу одним лишь фильтром ссылок.
    pub first_line: usize,
    pub lines: Vec<HitLine>,
}

/// Заметки одной папки (группировка макета § Поиск).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SearchGroup {
    pub folder: String,
    pub matches: usize,
    pub hits: Vec<SearchHit>,
}

/// Что показывать.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SearchResults {
    pub groups: Vec<SearchGroup>,
    pub total_matches: usize,
    pub total_notes: usize,
    /// Заметок нашлось больше [`MAX_NOTES`]; показаны первые.
    pub truncated: bool,
}

/// Почему поиск не состоялся.
///
/// Единственная причина — неверное регулярное выражение: его пишет
/// пользователь, и сказать ему «поиск не удался» без текста ошибки значит
/// оставить его чинить вслепую.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum SearchError {
    BadPattern { message: String },
}

/// Ищет по всем заметкам vault.
///
/// Читает через `VaultBackend`, своего доступа к диску здесь нет (принцип I).
/// Доски и служебные файлы пропускаются: содержимое доски — непрозрачные для
/// ядра байты (фича 002), и «совпадение» в её JSON пользователю ничего не
/// говорит.
pub async fn search<B: VaultBackend>(
    backend: &B,
    query: &SearchQuery,
) -> Result<SearchResults, SearchError> {
    if query.is_empty() {
        return Ok(SearchResults::default());
    }

    let matcher = Matcher::build(query)?;

    // Фильтр по ссылкам спрашивается один раз, а не на каждую заметку: ответ
    // от заметки не зависит, а обратные ссылки — самая дорогая операция здесь.
    let linking: Option<Vec<String>> = match &query.links_to {
        Some(target) => Some(
            backend
                .backlinks(std::path::Path::new(target))
                .await
                .into_iter()
                .map(|note| note.path.to_string_lossy().into_owned())
                .collect(),
        ),
        None => None,
    };

    // Условия по свойствам считаются один раз по индексу: файлы не читаются.
    let by_properties: Option<std::collections::HashSet<String>> = if query.properties.is_empty() {
        None
    } else {
        let now = now_ms();
        Some(
            backend
                .indexed_notes()
                .await
                .into_iter()
                .filter(|note| matches_all(note, &query.properties, now))
                .map(|note| note.path)
                .collect(),
        )
    };

    let mut hits: Vec<SearchHit> = Vec::new();
    let mut total_matches = 0usize;
    let mut truncated = false;

    for note in backend.list().await {
        if note.kind != FileKind::Note {
            continue;
        }
        let path = note.path.to_string_lossy().into_owned();

        let links_ok = linking
            .as_ref()
            .map(|paths| paths.iter().any(|known| known == &path));

        // Строка запроса пустая — по ней условия нет вовсе; иначе читаем файл.
        let text_hit = if matcher.is_some() {
            let Ok(content) = backend.read(&note.path).await else {
                // Нечитаемый файл — не отказ поиска: остальные заметки найтись
                // обязаны. Пользователь узнает о нём из дерева, где такой файл
                // и так помечен.
                continue;
            };
            Some(scan(&content, matcher.as_ref().expect("matcher")))
        } else {
            None
        };

        let has_text = text_hit.as_ref().map(|found| found.matches > 0);
        let properties_ok = by_properties
            .as_ref()
            .map(|paths| paths.contains(&path.replace('\\', "/")));
        if !keep(has_text, links_ok, properties_ok, query.mode) {
            continue;
        }

        let found = text_hit.unwrap_or_default();
        total_matches += found.matches;

        if hits.len() >= MAX_NOTES {
            truncated = true;
            continue;
        }

        hits.push(SearchHit {
            folder: folder_of(&path),
            path,
            name: note.name.clone(),
            matches: found.matches,
            first_line: found.first_line,
            lines: found.lines,
        });
    }

    Ok(group(hits, total_matches, truncated))
}

/// Проходит ли заметка условия при выбранном режиме.
///
/// `None` означает «условие не задано» и в обоих режимах не участвует: при
/// «И» оно не может отказать, при «ИЛИ» — не может разрешить.
fn keep(
    text: Option<bool>,
    links: Option<bool>,
    properties: Option<bool>,
    mode: FilterMode,
) -> bool {
    match mode {
        FilterMode::And => {
            text.unwrap_or(true) && links.unwrap_or(true) && properties.unwrap_or(true)
        }
        FilterMode::Or => {
            text.unwrap_or(false) || links.unwrap_or(false) || properties.unwrap_or(false)
        }
    }
}

/// Строка запроса после выделения условий по свойствам.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ParsedConditions {
    /// Что осталось от строки: обычный текст поиска.
    pub text: String,
    pub conditions: Vec<Condition>,
}

/// Разделяет строку на слова по пробелам; слово в двойных кавычках (целиком или
/// после `ключ:`) может содержать пробелы.
fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in text.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                current.push(ch);
            }
            ch if ch.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            ch => current.push(ch),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .map(|rest| rest.strip_suffix('"').unwrap_or(rest))
        .unwrap_or(value)
        .to_string()
}

fn parse_date(text: &str) -> Option<Ymd> {
    let mut parts = text.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    (parts.next().is_none() && valid_date(year, month, day)).then_some(Ymd { year, month, day })
}

/// `N` суток либо `ГГГГ-ММ-ДД..ГГГГ-ММ-ДД`.
fn parse_range(text: &str) -> Option<DateRange> {
    if let Some((from, to)) = text.split_once("..") {
        return Some(DateRange::Between {
            from: parse_date(from)?,
            to: parse_date(to)?,
        });
    }
    text.parse::<u32>()
        .ok()
        .map(|days| DateRange::LastDays { days })
}

/// Значение поля: дата, число, флажок или текст.
fn parse_value(text: &str) -> PropertyValue {
    let text = unquote(text);
    if let Some(date) = parse_date(&text) {
        return PropertyValue::Date {
            year: date.year,
            month: date.month,
            day: date.day,
        };
    }
    match text.as_str() {
        "true" | "да" => return PropertyValue::Flag { value: true },
        "false" | "нет" => return PropertyValue::Flag { value: false },
        _ => {}
    }
    match text.parse::<f64>() {
        Ok(value) if value.is_finite() => PropertyValue::Number { value },
        _ => PropertyValue::Text { value: text },
    }
}

/// Выделяет из набранной строки условия по свойствам.
///
/// Понимаются слова: `тег:X` (`tag:`), `папка:X` (`folder:`), `изменена:N` и
/// `изменена:ГГГГ-ММ-ДД..ГГГГ-ММ-ДД` (`modified:`; так же `создана:`), а для
/// ключей из `known_keys` — `ключ:значение`, `ключ:>значение` (также `<`,
/// `>=`, `<=`, `!=`) и `ключ:*` («задано»). Всё остальное, в том числе
/// `время:10:30` с неизвестным ключом, остаётся обычным текстом поиска.
pub fn parse_conditions(text: &str, known_keys: &[String]) -> ParsedConditions {
    let mut rest: Vec<String> = Vec::new();
    let mut conditions: Vec<Condition> = Vec::new();
    for word in words(text) {
        let parsed = word
            .split_once(':')
            .filter(|(key, value)| !key.is_empty() && !value.is_empty())
            .and_then(|(key, value)| parse_word(key, value, known_keys));
        match parsed {
            Some(condition) => conditions.push(condition),
            None => rest.push(word),
        }
    }
    ParsedConditions {
        text: rest.join(" "),
        conditions,
    }
}

fn parse_word(key: &str, value: &str, known_keys: &[String]) -> Option<Condition> {
    match key {
        "тег" | "tag" => Some(Condition::TagHas {
            tag: unquote(value),
        }),
        "папка" | "folder" => Some(Condition::Folder {
            folder: unquote(value),
        }),
        "изменена" | "modified" => Some(Condition::Modified {
            range: parse_range(value)?,
        }),
        "создана" | "created" => Some(Condition::Created {
            range: parse_range(value)?,
        }),
        key if known_keys.iter().any(|known| known == key) => {
            if value == "*" {
                return Some(Condition::Exists {
                    key: key.to_string(),
                });
            }
            let (op, raw) = [
                (">=", CompareOp::Ge),
                ("<=", CompareOp::Le),
                ("!=", CompareOp::Ne),
                (">", CompareOp::Gt),
                ("<", CompareOp::Lt),
            ]
            .into_iter()
            .find_map(|(prefix, op)| value.strip_prefix(prefix).map(|raw| (op, raw)))
            .unwrap_or((CompareOp::Eq, value));
            (!raw.is_empty()).then(|| Condition::Compare {
                key: key.to_string(),
                op,
                value: parse_value(raw),
            })
        }
        _ => None,
    }
}

/// Папка заметки — всё до последнего разделителя.
fn folder_of(path: &str) -> String {
    match path.rfind('/') {
        Some(cut) => path[..cut].to_owned(),
        None => String::new(),
    }
}

#[derive(Default)]
struct Found {
    matches: usize,
    first_line: usize,
    lines: Vec<HitLine>,
}

/// Скомпилированное условие по тексту.
///
/// Подстрока — тоже регулярное выражение, из экранированной строки: иначе
/// пришлось бы писать второй поиск совпадений и второй разбор регистра, а
/// расходиться этим двум путям нельзя.
struct Matcher(regex::Regex);

impl Matcher {
    fn build(query: &SearchQuery) -> Result<Option<Self>, SearchError> {
        let text = query.text.trim();
        if text.is_empty() {
            return Ok(None);
        }
        let pattern = if query.regex {
            text.to_owned()
        } else {
            regex::escape(text)
        };
        regex::RegexBuilder::new(&pattern)
            .case_insensitive(!query.case_sensitive)
            .build()
            .map(|compiled| Some(Self(compiled)))
            .map_err(|error| SearchError::BadPattern {
                message: error.to_string(),
            })
    }
}

/// Собирает совпадения по строкам файла вместе с соседними строками.
fn scan(content: &str, matcher: &Matcher) -> Found {
    let lines: Vec<&str> = content.lines().collect();
    let mut found = Found::default();
    let mut shown = 0usize;
    // Номер последней показанной строки: соседняя строка двух совпадений
    // подряд не должна попасть в выдачу дважды.
    let mut last_shown: Option<usize> = None;

    for (index, line) in lines.iter().enumerate() {
        let segments = split(line, matcher);
        let count = segments.iter().filter(|part| part.highlighted).count();
        if count == 0 {
            continue;
        }

        found.matches += count;
        if found.first_line == 0 {
            found.first_line = index + 1;
        }
        if shown >= MAX_HITS_PER_NOTE {
            continue;
        }
        shown += 1;

        // Строка до, сама строка, строка после — контекст макета.
        for offset in [-1i64, 0, 1] {
            let Some(number) = (index as i64 + offset)
                .try_into()
                .ok()
                .filter(|n: &usize| *n < lines.len())
            else {
                continue;
            };
            if last_shown.is_some_and(|last| last >= number + 1) {
                continue;
            }
            let is_match = offset == 0;
            found.lines.push(HitLine {
                number: number + 1,
                segments: if is_match {
                    segments.clone()
                } else {
                    vec![LineSegment {
                        text: lines[number].to_string(),
                        highlighted: false,
                    }]
                },
                is_match,
            });
            last_shown = Some(number + 1);
        }
    }

    found
}

/// Режет строку на куски по совпадениям.
fn split(line: &str, matcher: &Matcher) -> Vec<LineSegment> {
    let mut segments: Vec<LineSegment> = Vec::new();
    let mut cursor = 0usize;

    for found in matcher.0.find_iter(line) {
        // Пустое совпадение (например, `a*`) не двигало бы курсор и дало бы
        // бесконечный список кусков — такие пропускаем.
        if found.start() == found.end() {
            continue;
        }
        if found.start() > cursor {
            segments.push(LineSegment {
                text: line[cursor..found.start()].to_owned(),
                highlighted: false,
            });
        }
        segments.push(LineSegment {
            text: found.as_str().to_owned(),
            highlighted: true,
        });
        cursor = found.end();
    }

    if segments.is_empty() {
        return Vec::new();
    }
    if cursor < line.len() {
        segments.push(LineSegment {
            text: line[cursor..].to_owned(),
            highlighted: false,
        });
    }
    segments
}

/// Складывает найденное в группы по папкам, сохраняя порядок обхода vault.
fn group(hits: Vec<SearchHit>, total_matches: usize, truncated: bool) -> SearchResults {
    let total_notes = hits.len();
    let mut groups: Vec<SearchGroup> = Vec::new();

    for hit in hits {
        match groups.iter_mut().find(|group| group.folder == hit.folder) {
            Some(group) => {
                group.matches += hit.matches;
                group.hits.push(hit);
            }
            None => groups.push(SearchGroup {
                folder: hit.folder.clone(),
                matches: hit.matches,
                hits: vec![hit],
            }),
        }
    }

    SearchResults {
        groups,
        total_matches,
        total_notes,
        truncated,
    }
}
