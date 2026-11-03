//! Файлы графа знаний (фича 054, research.md §1, data-model.md): три файла JSON Lines в
//! `<папка уроков>/Граф знаний/`, только дописывание. Удаление — строка-«надгробие».
//!
//! Файлы приходят и с синхронизацией, то есть могут быть враждебными, поэтому чтение
//! линейное и с пределами: размер файла, длина строки, число строк, число формулировок и
//! цепочек; нечитаемая строка пропускается. Запись атомарна (`write_bytes`) и не идёт
//! через символические ссылки. Порядок «раньше — позже» между файлами задаёт время строки
//! `at`: надгробие действует на строки не позже себя, поздняя строка оживляет.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::ai::settings::AiSettings;
use crate::ai::AiFailure;
use crate::vault::backend::{ReadError, VaultBackend};

/// Папка графа внутри папки уроков.
pub const FOLDER_NAME: &str = "Граф знаний";
pub const PROPOSITIONS_FILE: &str = "propositions.jsonl";
pub const CHAINS_FILE: &str = "chains.jsonl";
pub const SOURCES_FILE: &str = "sources.jsonl";

/// Предел размера файла графа: больше — файл не читается и в него не пишут.
pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;
/// Строка длиннее пропускается, не разбираясь.
pub const MAX_LINE_BYTES: usize = 8 * 1024;
/// Строк на файл, которые разбираются.
pub const MAX_LINES: usize = 200_000;
/// Длина формулировки, символов.
pub const TEXT_MAX_CHARS: usize = 300;
/// Путь источника, символов.
pub const SOURCE_MAX_CHARS: usize = 300;
/// Формулировок одного утверждения и происхождений одной цепочки, которые учитываются.
pub const MAX_PHRASINGS: usize = 12;
/// Посылок в цепочке.
pub const MAX_PREMISES: usize = 6;
/// Утверждений и цепочек в графе, сверх которых новые строки игнорируются.
pub const MAX_PROPOSITIONS: usize = 50_000;
pub const MAX_CHAINS: usize = 50_000;
/// Переносов источников, которые учитываются: последние (ревью 054, M2).
const MAX_MOVES: usize = 256;
/// Запас файла под удаления: новые утверждения и покрытие пишутся, пока файл меньше
/// `MAX_FILE_BYTES - RESERVE_BYTES`, надгробия и переносы — до самого предела (L8).
pub const RESERVE_BYTES: usize = 256 * 1024;

/// Откуда взято: урок или (позже) заметка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SourceKind {
    Lesson,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "line")]
pub enum PropositionLine {
    Phrasing {
        id: String,
        text: String,
        source: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        event: Option<u32>,
        at: i64,
    },
    Removed {
        id: String,
        at: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "line")]
pub enum ChainLine {
    Chain {
        id: String,
        premises: Vec<String>,
        conclusion: String,
        source: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        event: Option<u32>,
        at: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "line")]
pub enum SourceLine {
    /// События урока с номерами меньше `upto` разобраны.
    Covered {
        source: String,
        kind: SourceKind,
        upto: u32,
        at: i64,
    },
    /// Вклад источника убран (`keep: false`) или источник удалён, а вклад оставлен.
    SourceRemoved { source: String, keep: bool, at: i64 },
    /// Источник (или папка с источниками) перенесён; строки до переноса относятся к новому
    /// пути.
    Moved { from: String, to: String, at: i64 },
}

fn line_of<T: Serialize>(value: &T) -> Result<String, AiFailure> {
    serde_json::to_string(value).map_err(|_| AiFailure::JournalFailed)
}

// ── Проекция ────────────────────────────────────────────────────────────────

/// Формулировка утверждения и откуда она.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrasing {
    pub text: String,
    pub source: String,
    pub event: Option<u32>,
    pub at: i64,
}

/// Происхождение цепочки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub source: String,
    pub event: Option<u32>,
    pub at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PropositionEntry {
    pub phrasings: Vec<Phrasing>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainEntry {
    pub premises: Vec<String>,
    pub conclusion: String,
    pub origins: Vec<Origin>,
}

/// Граф, каким его видит читатель: надгробия применены, источники приведены к текущим
/// путям.
#[derive(Debug, Clone, Default)]
pub struct Projection {
    pub propositions: HashMap<String, PropositionEntry>,
    pub chains: HashMap<String, ChainEntry>,
    /// Источник -> число покрытых событий.
    pub covered: HashMap<String, u32>,
    /// Источники, удалённые с сохранением вклада.
    pub removed_sources: HashSet<String>,
    /// Файлы графа не читаются целиком (слишком большие, ссылка, сбой чтения).
    pub damaged: bool,
    /// Покрытие по пути в свёрнутой форме `check_access` (ревью 054, L9): `Урок.lesson` и
    /// `урок.lesson` — один файл на файловой системе без учёта регистра.
    covered_folded: HashMap<String, u32>,
    /// Самое позднее время строки по id утверждения (в том числе скрытой надгробием).
    pub latest_by_id: HashMap<String, i64>,
    /// То же по источнику (путь после переносов).
    pub latest_by_source: HashMap<String, i64>,
    /// Самое позднее время среди учтённых переносов.
    pub last_move_at: i64,
}

/// Время новой строки: не раньше `now` и строго позже всего, что уже записано (`floor`).
/// Так надгробие и покрытие не оказываются «раньше» чужой строки из-за часов другого
/// устройства (ревью 054, L1).
pub fn stamp_after(now: i64, floor: Option<i64>) -> i64 {
    floor.map_or(now, |floor| now.max(floor.saturating_add(1)))
}

/// Путь для сравнения: компоненты в свёрнутой форме `check_access`.
pub fn folded_key(path: &str) -> String {
    match crate::ai::access::folded(Path::new(path)) {
        Some(parts) => parts.join("/"),
        None => path.to_owned(),
    }
}

/// Идентификатор утверждения или цепочки: `p_`/`c_` и шестнадцатеричный хэш.
pub fn is_valid_id(id: &str, prefix: &str) -> bool {
    id.len() <= 66
        && id.len() > prefix.len() + 7
        && id.starts_with(prefix)
        && id[prefix.len()..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn clip(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Формулировка в одной форме для сравнения: NFC, без регистра, пробелы схлопнуты, без
/// хвостовой пунктуации и кавычек по краям.
pub fn normalize_text(text: &str) -> String {
    let lowered: String = text.nfc().flat_map(char::to_lowercase).collect();
    let squeezed = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    squeezed
        .trim_matches(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '.' | ',' | ';' | ':' | '!' | '?' | '…' | '"' | '\'' | '«' | '»' | '“' | '”'
                )
        })
        .to_owned()
}

fn hash16(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().as_str()[..16].to_owned()
}

/// `p_<хэш нормализованного текста>`.
pub fn proposition_id(text: &str) -> String {
    format!("p_{}", hash16(&normalize_text(text)))
}

/// `c_<хэш отсортированных посылок и вывода>`.
pub fn chain_id(premises: &[String], conclusion: &str) -> String {
    let mut sorted: Vec<&str> = premises.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    sorted.dedup();
    format!(
        "c_{}",
        hash16(&format!("{}=>{conclusion}", sorted.join(",")))
    )
}

struct Move {
    from: Vec<String>,
    to: Vec<String>,
    at: i64,
}

fn components(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(str::to_owned)
        .collect()
}

/// Пути источников после переносов. Переносы разложены по первому компоненту `from`, путь
/// разбирается один раз и переписывается только при совпадении; результат запоминается
/// (ревью 054, M3): враждебный `sources.jsonl` не даёт «переносы × строки» с выделением
/// памяти на каждый шаг.
struct Resolver {
    moves: Vec<Move>,
    by_first: HashMap<String, Vec<usize>>,
    cache: HashMap<(String, i64), String>,
}

impl Resolver {
    fn new(moves: Vec<Move>) -> Self {
        let mut by_first: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, step) in moves.iter().enumerate() {
            by_first
                .entry(step.from[0].clone())
                .or_default()
                .push(index);
        }
        Resolver {
            moves,
            by_first,
            cache: HashMap::new(),
        }
    }

    /// Путь источника после переносов, записанных не раньше самой строки.
    fn resolve(&mut self, source: &str, at: i64) -> String {
        if self.moves.is_empty() {
            return source.to_owned();
        }
        let key = (source.to_owned(), at);
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone();
        }
        let mut parts = components(source);
        let mut cursor = 0usize;
        let mut moved = false;
        while let Some(bucket) = parts.first().and_then(|first| self.by_first.get(first)) {
            let start = bucket.partition_point(|index| *index < cursor);
            let hit = bucket[start..].iter().copied().find(|index| {
                let step = &self.moves[*index];
                step.at >= at && parts.starts_with(&step.from)
            });
            let Some(index) = hit else { break };
            let step = &self.moves[index];
            let rest = parts.split_off(step.from.len());
            parts = step.to.clone();
            parts.extend(rest);
            cursor = index + 1;
            moved = true;
        }
        let out = if moved {
            parts.join("/")
        } else {
            source.to_owned()
        };
        if self.cache.len() < MAX_LINES {
            self.cache.insert(key, out.clone());
        }
        out
    }
}

/// Поднимает самое позднее время по ключу.
fn raise(latest: &mut HashMap<String, i64>, key: &str, at: i64) {
    let entry = latest.entry(key.to_owned()).or_insert(i64::MIN);
    *entry = (*entry).max(at);
}

fn lines_of(bytes: &[u8]) -> impl Iterator<Item = &str> {
    bytes
        .split(|byte| *byte == b'\n')
        .take(MAX_LINES)
        .filter(|line| !line.is_empty() && line.len() <= MAX_LINE_BYTES)
        .filter_map(|line| std::str::from_utf8(line).ok())
}

fn parse_lines<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Vec<T> {
    lines_of(bytes)
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Собирает проекцию из содержимого трёх файлов. Линейна по числу строк.
pub fn project(propositions: &[u8], chains: &[u8], sources: &[u8]) -> Projection {
    let source_lines: Vec<SourceLine> = parse_lines(sources);
    let mut moves: Vec<Move> = Vec::new();
    for line in &source_lines {
        if let SourceLine::Moved { from, to, at } = line {
            let from = components(from);
            let to = components(to);
            if !from.is_empty() && !to.is_empty() {
                moves.push(Move { from, to, at: *at });
            }
        }
    }
    // Учитываются последние переносы: первые 256 не должны вытеснять свежие (M2).
    let surplus = moves.len().saturating_sub(MAX_MOVES);
    moves.drain(..surplus);
    let mut out = Projection {
        last_move_at: moves.iter().map(|step| step.at).max().unwrap_or(0),
        ..Projection::default()
    };
    let mut resolver = Resolver::new(moves);
    // Время, не позже которого вклад источника убран.
    let mut dropped: HashMap<String, i64> = HashMap::new();
    // Покрытие: побеждает строка с наибольшим `at` (при равных — более поздняя в файле),
    // а не максимум `upto`: урок, пересозданный по тому же пути короче прежнего, иначе
    // никогда не стал бы покрытым (H1).
    let mut covered_at: HashMap<String, (i64, u32)> = HashMap::new();
    for line in &source_lines {
        match line {
            SourceLine::Covered {
                source, upto, at, ..
            } => {
                let source = resolver.resolve(&clip(source, SOURCE_MAX_CHARS), *at);
                raise(&mut out.latest_by_source, &source, *at);
                let entry = covered_at
                    .entry(folded_key(&source))
                    .or_insert((i64::MIN, 0));
                if *at >= entry.0 {
                    *entry = (*at, *upto);
                }
                out.covered.entry(source).or_insert(0);
            }
            SourceLine::SourceRemoved { source, keep, at } => {
                let source = resolver.resolve(&clip(source, SOURCE_MAX_CHARS), *at);
                raise(&mut out.latest_by_source, &source, *at);
                if *keep {
                    out.removed_sources.insert(source);
                } else {
                    let entry = dropped.entry(source).or_insert(i64::MIN);
                    *entry = (*entry).max(*at);
                }
            }
            SourceLine::Moved { .. } => {}
        }
    }
    out.covered_folded = covered_at
        .into_iter()
        .map(|(key, (_, upto))| (key, upto))
        .collect();
    for (source, upto) in out.covered.iter_mut() {
        *upto = out
            .covered_folded
            .get(&folded_key(source))
            .copied()
            .unwrap_or(0);
    }

    let proposition_lines: Vec<PropositionLine> = parse_lines(propositions);
    let mut removed_at: HashMap<&str, i64> = HashMap::new();
    for line in &proposition_lines {
        if let PropositionLine::Removed { id, at } = line {
            let entry = removed_at.entry(id.as_str()).or_insert(i64::MIN);
            *entry = (*entry).max(*at);
            if is_valid_id(id, "p_") {
                raise(&mut out.latest_by_id, id, *at);
            }
        }
    }
    for line in &proposition_lines {
        let PropositionLine::Phrasing {
            id,
            text,
            source,
            event,
            at,
        } = line
        else {
            continue;
        };
        if !is_valid_id(id, "p_") || text.trim().is_empty() {
            continue;
        }
        raise(&mut out.latest_by_id, id, *at);
        if removed_at
            .get(id.as_str())
            .is_some_and(|removed| *removed >= *at)
        {
            continue;
        }
        let source = resolver.resolve(&clip(source, SOURCE_MAX_CHARS), *at);
        raise(&mut out.latest_by_source, &source, *at);
        if dropped.get(&source).is_some_and(|removed| *removed >= *at) {
            continue;
        }
        if !out.propositions.contains_key(id) && out.propositions.len() >= MAX_PROPOSITIONS {
            continue;
        }
        let entry = out.propositions.entry(id.clone()).or_default();
        if entry.phrasings.len() < MAX_PHRASINGS {
            entry.phrasings.push(Phrasing {
                text: clip(text.trim(), TEXT_MAX_CHARS),
                source,
                event: *event,
                at: *at,
            });
        }
    }

    for line in parse_lines::<ChainLine>(chains) {
        let ChainLine::Chain {
            id,
            premises,
            conclusion,
            source,
            event,
            at,
        } = line;
        if !is_valid_id(&id, "c_")
            || !is_valid_id(&conclusion, "p_")
            || premises.is_empty()
            || premises.len() > MAX_PREMISES
            || premises.iter().any(|premise| !is_valid_id(premise, "p_"))
        {
            continue;
        }
        let source = resolver.resolve(&clip(&source, SOURCE_MAX_CHARS), at);
        raise(&mut out.latest_by_source, &source, at);
        if dropped.get(&source).is_some_and(|removed| *removed >= at) {
            continue;
        }
        if !out.chains.contains_key(&id) && out.chains.len() >= MAX_CHAINS {
            continue;
        }
        let entry = out.chains.entry(id).or_insert_with(|| ChainEntry {
            premises,
            conclusion,
            origins: Vec::new(),
        });
        if entry.origins.len() < MAX_PHRASINGS {
            entry.origins.push(Origin { source, event, at });
        }
    }
    out
}

impl Projection {
    /// Сколько событий урока покрыто; путь сравнивается в свёрнутой форме (L9).
    pub fn covered_of(&self, path: &str) -> u32 {
        self.covered_folded
            .get(&folded_key(path))
            .copied()
            .unwrap_or(0)
    }

    /// Есть ли покрытие у пути (в любой записи регистра).
    pub fn has_coverage(&self, path: &str) -> bool {
        self.covered_folded.contains_key(&folded_key(path))
    }

    /// Источники всех учтённых строк, по порядку.
    pub fn source_paths(&self) -> Vec<String> {
        let mut paths: HashSet<&str> = HashSet::new();
        for entry in self.propositions.values() {
            for phrasing in &entry.phrasings {
                paths.insert(&phrasing.source);
            }
        }
        for entry in self.chains.values() {
            for origin in &entry.origins {
                paths.insert(&origin.source);
            }
        }
        let mut paths: Vec<String> = paths.into_iter().map(str::to_owned).collect();
        paths.sort();
        paths
    }
}

// ── Файловая система ────────────────────────────────────────────────────────

/// Папка графа внутри папки уроков. Папка уроков уже очищена настройками и не может быть
/// служебным местом.
pub fn graph_folder(settings: &AiSettings) -> PathBuf {
    Path::new(&settings.lessons_folder).join(FOLDER_NAME)
}

pub fn graph_folder_string(settings: &AiSettings) -> String {
    graph_folder(settings).to_string_lossy().replace('\\', "/")
}

enum FileRead {
    Absent,
    Bytes(Vec<u8>),
    /// Не читается или небезопасен: ссылка, слишком большой, сбой.
    Unusable,
}

async fn read_file<B: VaultBackend>(backend: &B, path: &Path) -> FileRead {
    if backend.crosses_symlink(path).await || backend.is_symlink(path).await {
        return FileRead::Unusable;
    }
    match backend.read_bytes(path).await {
        Ok(bytes) if bytes.len() <= MAX_FILE_BYTES => FileRead::Bytes(bytes),
        Ok(_) => FileRead::Unusable,
        Err(ReadError::NotFound) => FileRead::Absent,
        Err(_) => FileRead::Unusable,
    }
}

/// Читает граф. Нечитаемый файл считается пустым, а проекция помечается `damaged`.
pub async fn load<B: VaultBackend>(backend: &B, settings: &AiSettings) -> Projection {
    let folder = graph_folder(settings);
    let mut damaged = false;
    let mut bytes: Vec<Vec<u8>> = Vec::new();
    for name in [PROPOSITIONS_FILE, CHAINS_FILE, SOURCES_FILE] {
        match read_file(backend, &folder.join(name)).await {
            FileRead::Bytes(content) => bytes.push(content),
            FileRead::Absent => bytes.push(Vec::new()),
            FileRead::Unusable => {
                damaged = true;
                bytes.push(Vec::new());
            }
        }
    }
    let mut projection = project(&bytes[0], &bytes[1], &bytes[2]);
    projection.damaged = damaged;
    projection
}

/// Дописывает строки в конец файла графа. Файл читается заново и переписывается целиком
/// атомарно; оборванная последняя строка отделяется переводом строки. Ссылка на пути
/// файла или папки, нечитаемый или слишком большой файл — отказ без записи.
pub async fn append<B: VaultBackend>(
    backend: &mut B,
    settings: &AiSettings,
    file: &str,
    lines: &[String],
) -> Result<(), AiFailure> {
    append_within(
        backend,
        settings,
        file,
        lines,
        MAX_FILE_BYTES - RESERVE_BYTES,
    )
    .await
}

/// То же с пределом размера: удаления (надгробия, переносы) пишутся до `MAX_FILE_BYTES`,
/// новое содержимое — с запасом под них (L8).
async fn append_within<B: VaultBackend>(
    backend: &mut B,
    settings: &AiSettings,
    file: &str,
    lines: &[String],
    limit: usize,
) -> Result<(), AiFailure> {
    if lines.is_empty() {
        return Ok(());
    }
    let folder = graph_folder(settings);
    let path = folder.join(file);
    if backend.crosses_symlink(&path).await
        || backend.is_symlink(&path).await
        || backend.is_symlink(&folder).await
    {
        return Err(AiFailure::JournalFailed);
    }
    backend
        .ensure_folder(&folder)
        .await
        .map_err(|_| AiFailure::JournalFailed)?;
    let mut bytes = match read_file(backend, &path).await {
        FileRead::Absent => Vec::new(),
        FileRead::Bytes(bytes) => bytes,
        FileRead::Unusable => return Err(AiFailure::JournalFailed),
    };
    if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    for line in lines {
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
    }
    if bytes.len() > limit {
        return Err(AiFailure::JournalFailed);
    }
    backend
        .write_bytes(&path, &bytes)
        .await
        .map_err(|_| AiFailure::JournalFailed)
}

pub async fn append_propositions<B: VaultBackend>(
    backend: &mut B,
    settings: &AiSettings,
    lines: &[PropositionLine],
) -> Result<(), AiFailure> {
    let only_removals = lines
        .iter()
        .all(|line| matches!(line, PropositionLine::Removed { .. }));
    let lines = lines.iter().map(line_of).collect::<Result<Vec<_>, _>>()?;
    let limit = if only_removals {
        MAX_FILE_BYTES
    } else {
        MAX_FILE_BYTES - RESERVE_BYTES
    };
    append_within(backend, settings, PROPOSITIONS_FILE, &lines, limit).await
}

pub async fn append_chains<B: VaultBackend>(
    backend: &mut B,
    settings: &AiSettings,
    lines: &[ChainLine],
) -> Result<(), AiFailure> {
    let lines = lines.iter().map(line_of).collect::<Result<Vec<_>, _>>()?;
    append(backend, settings, CHAINS_FILE, &lines).await
}

pub async fn append_sources<B: VaultBackend>(
    backend: &mut B,
    settings: &AiSettings,
    lines: &[SourceLine],
) -> Result<(), AiFailure> {
    let only_removals = lines
        .iter()
        .all(|line| !matches!(line, SourceLine::Covered { .. }));
    let lines = lines.iter().map(line_of).collect::<Result<Vec<_>, _>>()?;
    let limit = if only_removals {
        MAX_FILE_BYTES
    } else {
        MAX_FILE_BYTES - RESERVE_BYTES
    };
    append_within(backend, settings, SOURCES_FILE, &lines, limit).await
}

/// Источник переехал вместе с уроком, заметкой или папкой: строка `Moved` в
/// `sources.jsonl`. Файла графа нет — писать нечего. Если переехала сама папка с графом,
/// строка пишется в файл на новом месте. Сбой записи перенос не отменяет.
pub async fn move_source<B: VaultBackend>(backend: &mut B, from: &Path, to: &Path, now: i64) {
    let settings = crate::ai::settings::load_settings(backend).await;
    let from_text = from.to_string_lossy().replace('\\', "/");
    let to_text = to.to_string_lossy().replace('\\', "/");
    let folder = graph_folder_string(&settings);
    let existing = match read_file(backend, &graph_folder(&settings).join(SOURCES_FILE)).await {
        FileRead::Absent => None,
        FileRead::Bytes(_) => Some(settings.clone()),
        FileRead::Unusable => return,
    };
    let target = match existing {
        Some(settings) => settings,
        None => {
            // Папка графа могла поехать вместе с родителем: файл ищется на новом месте.
            let parts = components(&folder);
            let moved_from = components(&from_text);
            if !parts.starts_with(&moved_from) {
                return;
            }
            let rest = parts[moved_from.len()..].join("/");
            let new_folder = if rest.is_empty() {
                to_text.clone()
            } else {
                format!("{to_text}/{rest}")
            };
            let Some(lessons) = new_folder
                .strip_suffix(&format!("/{FOLDER_NAME}"))
                .map(str::to_owned)
            else {
                return;
            };
            let mut moved = settings.clone();
            moved.lessons_folder = lessons;
            if !matches!(
                read_file(backend, &graph_folder(&moved).join(SOURCES_FILE)).await,
                FileRead::Bytes(_)
            ) {
                return;
            }
            moved
        }
    };
    // `Moved` пишется, только если `from` — путь или префикс известного источника: перенос
    // любого другого файла не должен вытеснять нужные переносы (M2). Время строки позже
    // всех строк этих источников и тех же часов, что вся запись (L1).
    let projection = load(backend, &target).await;
    if projection.damaged {
        return;
    }
    let Some(from_parts) = crate::ai::access::folded(from) else {
        return;
    };
    let mut floor: Option<i64> = None;
    for (source, at) in &projection.latest_by_source {
        if crate::ai::access::folded(Path::new(source))
            .is_some_and(|parts| parts.starts_with(&from_parts))
        {
            floor = Some(floor.map_or(*at, |known| known.max(*at)));
        }
    }
    if floor.is_none() {
        return;
    }
    let line = SourceLine::Moved {
        from: from_text,
        to: to_text,
        at: stamp_after(now, floor),
    };
    let _ = append_sources(backend, &target, &[line]).await;
}

/// Читает только `sources.jsonl`: для признаков урока утверждения и цепочки не нужны.
pub async fn load_sources<B: VaultBackend>(backend: &B, settings: &AiSettings) -> Projection {
    let path = graph_folder(settings).join(SOURCES_FILE);
    let (bytes, damaged) = match read_file(backend, &path).await {
        FileRead::Bytes(bytes) => (bytes, false),
        FileRead::Absent => (Vec::new(), false),
        FileRead::Unusable => (Vec::new(), true),
    };
    let mut projection = project(&[], &[], &bytes);
    projection.damaged = damaged;
    projection
}
