//! Граф связей vault одним вызовом (фича 025, research.md §1–§5).
//!
//! Что в графе есть — какие узлы и связи, их виды, число связей, фильтр, цвет
//! группы, локальная выборка по глубине — решает ядро (принцип I). Интерфейс
//! только раскладывает и рисует то, что получил. Источник — индекс vault:
//! заметки и доски, разрешённые ссылки, вставки `#image`, теги из свойств 020.
//! Файлы при построении не читаются (принцип X).
//!
//! Построение — чистая функция [`build_graph`] над [`GraphSource`]; [`vault_graph`]
//! только берёт источник у backend. Поэтому правила проверяются одной таблицей
//! тестов на обоих таргетах без файловой системы.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::properties::index::IndexedNote;
use crate::properties::query::{matches_all, Condition};
use crate::properties::{now_ms, PropertyValue, TAGS_KEY};
use crate::search::{parse_conditions, ParsedConditions};
use crate::vault::backend::VaultBackend;
use crate::vault::FileKind;

/// Потолок глубины локального графа.
pub const MAX_DEPTH: u8 = 3;
/// Сколько групп цвета принимается: остальные отбрасываются.
pub const MAX_GROUPS: usize = 32;
/// Префикс идентификатора узла-тега.
pub const TAG_ID_PREFIX: &str = "tag:";

// ── Запрос ──────────────────────────────────────────────────────────────────

/// Цвет группы — имя токена палитры, а не произвольное значение: интерфейс
/// красит токенами проекта.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "lowercase")]
pub enum GroupColor {
    Orange,
    Blue,
    Green,
    Amber,
    Red,
    Hivis,
    Steel,
}

/// Группа: условие того же языка, что у поиска 020, и цвет.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct GraphGroup {
    pub query: String,
    pub color: GroupColor,
}

/// Локальный граф: центр и глубина.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LocalGraph {
    /// Идентификатор узла-центра (путь заметки или доски).
    pub center: String,
    /// 1..=3; вне диапазона приводится к нему.
    pub depth: u8,
}

/// Запрос графа (data-model.md «GraphRequest»).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct GraphRequest {
    /// Язык условий поиска 020: текст, `тег:`, `папка:`; пусто — без фильтра.
    pub filter: String,
    pub show_orphans: bool,
    pub show_attachments: bool,
    pub show_tags: bool,
    pub groups: Vec<GraphGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub local: Option<LocalGraph>,
}

// ── Ответ ───────────────────────────────────────────────────────────────────

/// Вид узла.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum GraphNodeKind {
    Note,
    Board,
    Attachment,
    Tag,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct GraphNode {
    /// Путь относительно корня vault; у тега — `tag:имя`.
    pub id: String,
    pub kind: GraphNodeKind,
    /// Название для подписи: имя без расширения (у вложения — с ним).
    pub label: String,
    /// Путь файла; у тега нет.
    pub path: Option<String>,
    /// Число связей среди видимого графа.
    pub degree: u32,
    /// Индекс первой подошедшей группы запроса.
    pub group: Option<u32>,
}

/// Связь между двумя узлами; одна запись на пару.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct GraphEdge {
    /// Индексы в `nodes`. Направление — от ссылающегося к цели; при
    /// `both_ways` направление условно (`from < to`).
    pub from: u32,
    pub to: u32,
    /// Ссылки есть в обе стороны.
    pub both_ways: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct VaultGraph {
    /// Видимые узлы по возрастанию `id`.
    pub nodes: Vec<GraphNode>,
    /// Видимые связи по возрастанию `(from, to)`.
    pub edges: Vec<GraphEdge>,
    /// Индекс центра локального графа в `nodes`.
    pub center: Option<u32>,
    /// Заметок и досок в vault до фильтра.
    pub total_notes: u32,
}

// ── Настройки графа (ключ `graph` в vault.json) ─────────────────────────────

/// Силы раскладки, каждая в пределах 0..=1; значения на параметры раскладки
/// отображает интерфейс.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct GraphForces {
    pub center: f64,
    pub repel: f64,
    pub link: f64,
    pub distance: f64,
}

impl Default for GraphForces {
    fn default() -> Self {
        Self {
            center: 0.3,
            repel: 0.6,
            link: 0.5,
            distance: 0.45,
        }
    }
}

/// Настройки графа, хранимые вместе с vault (data-model.md «GraphSettings»).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(default)]
pub struct GraphSettings {
    pub filter: String,
    pub show_orphans: bool,
    pub show_attachments: bool,
    pub show_tags: bool,
    pub arrows: bool,
    pub groups: Vec<GraphGroup>,
    pub forces: GraphForces,
    /// Масштаб, с которого видны подписи (0..=1).
    pub label_zoom: f64,
    /// Множитель размера узлов (0..=1).
    pub node_size: f64,
}

impl Default for GraphSettings {
    fn default() -> Self {
        Self {
            filter: String::new(),
            show_orphans: true,
            show_attachments: false,
            show_tags: false,
            arrows: false,
            groups: Vec::new(),
            forces: GraphForces::default(),
            label_zoom: 0.4,
            node_size: 0.5,
        }
    }
}

fn unit(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

impl GraphSettings {
    /// Приводит значения к допустимым: числа — в 0..=1 (нечисло — умолчание),
    /// групп не больше [`MAX_GROUPS`], группы с пустым условием отбрасываются.
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.forces = GraphForces {
            center: unit(self.forces.center, defaults.forces.center),
            repel: unit(self.forces.repel, defaults.forces.repel),
            link: unit(self.forces.link, defaults.forces.link),
            distance: unit(self.forces.distance, defaults.forces.distance),
        };
        self.label_zoom = unit(self.label_zoom, defaults.label_zoom);
        self.node_size = unit(self.node_size, defaults.node_size);
        self.groups.retain(|group| !group.query.trim().is_empty());
        self.groups.truncate(MAX_GROUPS);
        self
    }

    /// Запрос графа по настройкам; `local` задаёт окно, а не настройки.
    pub fn request(&self, local: Option<LocalGraph>) -> GraphRequest {
        GraphRequest {
            filter: self.filter.clone(),
            show_orphans: self.show_orphans,
            show_attachments: self.show_attachments,
            show_tags: self.show_tags,
            groups: self.groups.clone(),
            local,
        }
    }
}

// ── Источник ────────────────────────────────────────────────────────────────

/// Что граф берёт у индекса vault.
#[derive(Debug, Clone, Default)]
pub struct GraphSource {
    /// Заметки со свойствами.
    pub notes: Vec<IndexedNote>,
    /// Доски (свойств у них нет).
    pub boards: Vec<IndexedNote>,
    /// Разрешённые ссылки: источник -> цели. Цели могут не существовать.
    pub links: HashMap<PathBuf, Vec<PathBuf>>,
    /// Вставленные файлы: заметка -> пути `#image`.
    pub embeds: HashMap<PathBuf, Vec<PathBuf>>,
}

/// Источник без индекса: читает все заметки и разбирает их заново. Умолчание
/// [`VaultBackend::graph_source`], верное для любого backend, но медленное.
pub async fn scan_graph_source<B: VaultBackend>(backend: &B) -> GraphSource {
    let mut source = GraphSource {
        notes: backend.indexed_notes().await,
        ..GraphSource::default()
    };
    for note in backend.list().await {
        if note.kind == FileKind::Board {
            source
                .boards
                .push(IndexedNote::new(&note.path, &note.name, None));
        }
    }
    for note in &source.notes {
        let path = PathBuf::from(&note.path);
        if let Ok(text) = backend.read(&path).await {
            let links = crate::graph::links_in(&path, &text);
            if !links.is_empty() {
                source.links.insert(path.clone(), links);
            }
            let embeds = crate::graph::embeds_in(&path, &text);
            if !embeds.is_empty() {
                source.embeds.insert(path, embeds);
            }
        }
    }
    source
}

/// Граф vault одним вызовом (`vault_graph` из контракта).
pub async fn vault_graph<B: VaultBackend>(backend: &B, request: &GraphRequest) -> VaultGraph {
    build_graph(&backend.graph_source().await, request, now_ms())
}

// ── Построение ──────────────────────────────────────────────────────────────

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn stem(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

fn tag_key(raw: &str) -> String {
    raw.trim().trim_start_matches('#').to_lowercase()
}

/// Теги заметки из свойства `tags` (список или одно слово).
fn tags_of(note: &IndexedNote) -> Vec<&str> {
    let mut out = Vec::new();
    for entry in note.properties.iter().filter(|entry| entry.key == TAGS_KEY) {
        match &entry.value {
            PropertyValue::List { values } => out.extend(values.iter().map(String::as_str)),
            PropertyValue::Text { value } => out.push(value.as_str()),
            _ => {}
        }
    }
    out.into_iter()
        .filter(|tag| !tag.trim().trim_start_matches('#').is_empty())
        .collect()
}

/// Условия фильтра или группы, разобранные один раз.
struct Query {
    /// Слова обычного текста, в нижнем регистре; каждое должно встретиться в
    /// имени или пути.
    words: Vec<String>,
    conditions: Vec<Condition>,
}

impl Query {
    fn parse(text: &str, known_keys: &[String]) -> Self {
        let ParsedConditions { text, conditions } = parse_conditions(text, known_keys);
        Self {
            words: text.split_whitespace().map(str::to_lowercase).collect(),
            conditions,
        }
    }

    fn is_empty(&self) -> bool {
        self.words.is_empty() && self.conditions.is_empty()
    }
}

/// Узел-кандидат до отбора.
struct Candidate<'a> {
    id: String,
    kind: GraphNodeKind,
    label: String,
    path: Option<String>,
    /// Сведения для условий: заметка из индекса или пустая запись.
    entry: Entry<'a>,
    /// Путь в нижнем регистре для текстового условия.
    haystack: String,
}

enum Entry<'a> {
    Indexed(&'a IndexedNote),
    /// Вложение: только путь и имя.
    Synthetic(IndexedNote),
    /// Тег: условия, кроме `тег:` с тем же именем и текста, не проходят.
    Tag(String),
}

impl Candidate<'_> {
    fn matches(&self, query: &Query, now: i64) -> bool {
        if query.is_empty() {
            return true;
        }
        if !query
            .words
            .iter()
            .all(|word| self.haystack.contains(word.as_str()))
        {
            return false;
        }
        match &self.entry {
            Entry::Indexed(note) => matches_all(note, &query.conditions, now),
            Entry::Synthetic(note) => matches_all(note, &query.conditions, now),
            Entry::Tag(key) => query.conditions.iter().all(|condition| match condition {
                Condition::TagHas { tag } => tag_key(tag) == *key,
                _ => false,
            }),
        }
    }
}

/// Строит граф по источнику и запросу. `now` — «сейчас» для условий по датам.
pub fn build_graph(source: &GraphSource, request: &GraphRequest, now: i64) -> VaultGraph {
    // ── Кандидаты: заметки, доски, затем вложения и теги ──
    let mut candidates: Vec<Candidate> = Vec::new();
    for note in &source.notes {
        candidates.push(Candidate {
            id: note.path.clone(),
            kind: GraphNodeKind::Note,
            label: stem(&note.name).to_string(),
            path: Some(note.path.clone()),
            haystack: note.path.to_lowercase(),
            entry: Entry::Indexed(note),
        });
    }
    for board in &source.boards {
        candidates.push(Candidate {
            id: board.path.clone(),
            kind: GraphNodeKind::Board,
            label: stem(&board.name).to_string(),
            path: Some(board.path.clone()),
            haystack: board.path.to_lowercase(),
            entry: Entry::Indexed(board),
        });
    }
    let total_notes = candidates.len() as u32;
    let mut index_of: HashMap<String, usize> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| (candidate.id.clone(), index))
        .collect();

    // Связи заметка -> заметка/доска; вложения и теги добавляются узлами по
    // мере надобности.
    let mut raw_edges: Vec<(usize, usize)> = Vec::new();
    let mut sources: Vec<(&PathBuf, &Vec<PathBuf>)> = source.links.iter().collect();
    sources.sort();
    for (from, targets) in sources {
        let Some(&from_index) = index_of.get(&slashed(from)) else {
            continue;
        };
        for target in targets {
            if let Some(&to_index) = index_of.get(&slashed(target)) {
                // Петля не рисуется.
                if to_index != from_index {
                    raw_edges.push((from_index, to_index));
                }
            }
        }
    }

    if request.show_attachments {
        let mut embeds: Vec<(&PathBuf, &Vec<PathBuf>)> = source.embeds.iter().collect();
        embeds.sort();
        for (from, targets) in embeds {
            let Some(&from_index) = index_of.get(&slashed(from)) else {
                continue;
            };
            for target in targets {
                let id = slashed(target);
                let to_index = *index_of.entry(id.clone()).or_insert_with(|| {
                    let name = file_name(&id).to_string();
                    candidates.push(Candidate {
                        label: name.clone(),
                        path: Some(id.clone()),
                        haystack: id.to_lowercase(),
                        entry: Entry::Synthetic(IndexedNote::new(target, &name, None)),
                        id: id.clone(),
                        kind: GraphNodeKind::Attachment,
                    });
                    candidates.len() - 1
                });
                if to_index != from_index {
                    raw_edges.push((from_index, to_index));
                }
            }
        }
    }

    if request.show_tags {
        for note in &source.notes {
            let Some(&from_index) = index_of.get(&note.path) else {
                continue;
            };
            for tag in tags_of(note) {
                let key = tag_key(tag);
                let id = format!("{TAG_ID_PREFIX}{key}");
                let to_index = *index_of.entry(id.clone()).or_insert_with(|| {
                    candidates.push(Candidate {
                        label: tag.trim().trim_start_matches('#').to_string(),
                        path: None,
                        haystack: key.clone(),
                        entry: Entry::Tag(key.clone()),
                        id: id.clone(),
                        kind: GraphNodeKind::Tag,
                    });
                    candidates.len() - 1
                });
                raw_edges.push((from_index, to_index));
            }
        }
    }

    // ── Центр локального графа ──
    let local = request.local.as_ref();
    let center = match local {
        Some(local) => match index_of.get(&local.center) {
            Some(&index) => Some(index),
            // Центра нет в графе — показывать нечего.
            None => {
                return VaultGraph {
                    total_notes,
                    ..VaultGraph::default()
                }
            }
        },
        None => None,
    };

    // ── Фильтр ──
    let needs_keys = request.filter.contains(':')
        || request.groups.iter().any(|group| group.query.contains(':'));
    let known_keys = if needs_keys {
        let mut keys: Vec<String> = source
            .notes
            .iter()
            .flat_map(|note| note.properties.iter().map(|entry| entry.key.clone()))
            .collect();
        keys.sort();
        keys.dedup();
        keys
    } else {
        Vec::new()
    };
    let filter = Query::parse(&request.filter, &known_keys);
    let mut visible: Vec<bool> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| Some(index) == center || candidate.matches(&filter, now))
        .collect();

    // Связь остаётся, если оба конца видимы; пара в обе стороны — одна запись.
    let mut directed: HashSet<(usize, usize)> = HashSet::new();
    for &(from, to) in &raw_edges {
        if visible[from] && visible[to] {
            directed.insert((from, to));
        }
    }

    // Вложение или тег без единой связи узлом не бывает: они существуют только
    // как конец связи.
    let mut degree = vec![0u32; candidates.len()];
    let mut pairs: HashSet<(usize, usize)> = HashSet::new();
    for &(from, to) in &directed {
        if pairs.insert((from.min(to), from.max(to))) {
            degree[from] += 1;
            degree[to] += 1;
        }
    }
    for (index, candidate) in candidates.iter().enumerate() {
        let linked_only = matches!(
            candidate.kind,
            GraphNodeKind::Attachment | GraphNodeKind::Tag
        );
        if Some(index) != center && (linked_only || !request.show_orphans) && degree[index] == 0 {
            visible[index] = false;
        }
    }

    // ── Локальная выборка: обход в ширину в обе стороны ──
    if let (Some(center), Some(local)) = (center, local) {
        let depth = local.depth.clamp(1, MAX_DEPTH) as usize;
        let mut neighbours: HashMap<usize, Vec<usize>> = HashMap::new();
        for &(a, b) in &pairs {
            if visible[a] && visible[b] {
                neighbours.entry(a).or_default().push(b);
                neighbours.entry(b).or_default().push(a);
            }
        }
        let mut reached: HashMap<usize, usize> = HashMap::from([(center, 0)]);
        let mut queue = VecDeque::from([center]);
        while let Some(current) = queue.pop_front() {
            let step = reached[&current];
            if step == depth {
                continue;
            }
            for &next in neighbours.get(&current).into_iter().flatten() {
                if let std::collections::hash_map::Entry::Vacant(slot) = reached.entry(next) {
                    slot.insert(step + 1);
                    queue.push_back(next);
                }
            }
        }
        for (index, flag) in visible.iter_mut().enumerate() {
            *flag = *flag && reached.contains_key(&index);
        }
    }

    // ── Вывод: узлы по id, связи по индексам ──
    let mut order: Vec<usize> = (0..candidates.len()).filter(|&i| visible[i]).collect();
    order.sort_by(|&a, &b| candidates[a].id.cmp(&candidates[b].id));
    let mut position = vec![u32::MAX; candidates.len()];
    for (place, &index) in order.iter().enumerate() {
        position[index] = place as u32;
    }

    let groups: Vec<Query> = request
        .groups
        .iter()
        .map(|group| Query::parse(&group.query, &known_keys))
        .collect();

    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut final_degree = vec![0u32; order.len()];
    for &(a, b) in &pairs {
        if !(visible[a] && visible[b]) {
            continue;
        }
        let forward = directed.contains(&(a, b));
        let backward = directed.contains(&(b, a));
        let (from, to) = if forward && !backward {
            (a, b)
        } else if backward && !forward {
            (b, a)
        } else {
            (a.min(b), a.max(b))
        };
        let (from, to) = (position[from], position[to]);
        final_degree[from as usize] += 1;
        final_degree[to as usize] += 1;
        edges.push(GraphEdge {
            from,
            to,
            both_ways: forward && backward,
        });
    }
    edges.sort_by_key(|edge| (edge.from, edge.to));

    let nodes: Vec<GraphNode> = order
        .iter()
        .enumerate()
        .map(|(place, &index)| {
            let candidate = &candidates[index];
            GraphNode {
                id: candidate.id.clone(),
                kind: candidate.kind,
                label: candidate.label.clone(),
                path: candidate.path.clone(),
                degree: final_degree[place],
                group: groups
                    .iter()
                    .position(|group| !group.is_empty() && candidate.matches(group, now))
                    .map(|group| group as u32),
            }
        })
        .collect();

    VaultGraph {
        nodes,
        edges,
        center: center.map(|index| position[index]),
        total_notes,
    }
}
