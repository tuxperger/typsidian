//! План урока (фича 052, research.md §3–§5): проверка структуры, которую присылает модель,
//! раскладка карты слоями и вид плана по событиям файла урока. Модель не хозяин плана:
//! структура с ошибкой не попадает ни в файл, ни в вид (SC-002), а число шагов, длины
//! идентификаторов и названий ограничены, как пределы вопроса учителя (051, L8).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::file::LessonEvent;

/// Сколько шагов может быть в плане.
pub const MAX_STEPS: usize = 40;
/// Пределы длины: идентификатор шага, название, причина перестройки.
const ID_MAX_CHARS: usize = 64;
const TITLE_MAX_CHARS: usize = 120;
const REASON_MAX_CHARS: usize = 300;

/// Шаг плана в том виде, в каком он лежит в файле урока.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PlanStep {
    pub id: String,
    pub title: String,
    /// Шаги, которые нужно пройти раньше этого.
    #[serde(default)]
    pub after: Vec<String>,
    /// Пользователь уже знает шаг (по замеру).
    #[serde(default)]
    pub known: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum PlanStepState {
    Done,
    Current,
    Todo,
    Known,
}

/// Шаг в виде урока: с состоянием и местом на карте. Сетка: `x` — слой (столбец, слева
/// направо по ходу зависимостей), `y` — строка в слое; клетки не повторяются.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PlanStepView {
    pub id: String,
    pub title: String,
    pub after: Vec<String>,
    pub known: bool,
    pub state: PlanStepState,
    /// Шаг появился в этой версии плана (у первой версии таких нет).
    pub added: bool,
    /// Номер события урока, с которого шаг стал текущим: ближайшая реплика учителя (или
    /// ответ) не позже события плана. `None`, если шаг текущим не был.
    pub anchor: Option<u32>,
    pub x: u32,
    pub y: u32,
}

/// Ребро карты: `from` нужно пройти раньше `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PlanEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PlanView {
    pub version: u32,
    pub goal: String,
    pub current: String,
    pub steps: Vec<PlanStepView>,
    pub edges: Vec<PlanEdge>,
    /// Размер сетки: число слоёв и строк в самом большом слое.
    pub cols: u32,
    pub rows: u32,
    /// Сколько раз план перестраивался (версии минус одна) к этой версии.
    pub changes: u32,
    /// Причина, с которой принята эта версия.
    pub last_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PlanVersionInfo {
    pub version: u32,
    pub at: i64,
    pub steps: u32,
    pub reason: Option<String>,
}

/// Почему план не принят. Текст для модели — `prompt::plan_error`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    Malformed,
    Empty,
    /// Больше [`MAX_STEPS`]: цель слишком широкая.
    TooMany,
    BadId,
    DuplicateId(String),
    UnknownRef {
        step: String,
        target: String,
    },
    Cycle,
    UnknownGoal,
    UnknownCurrent,
    /// `plan_progress` до первого плана.
    NoPlan,
}

/// Принятые аргументы `plan_set`: пределы длин уже применены.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSet {
    pub steps: Vec<PlanStep>,
    pub goal: String,
    pub current: String,
    pub reason: Option<String>,
}

/// Принятые аргументы `plan_progress`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanProgressArgs {
    pub current: Option<String>,
    pub done: Vec<String>,
}

fn clip(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn text_of(value: &Value) -> String {
    value.as_str().map(str::trim).unwrap_or_default().to_owned()
}

/// Разбирает и проверяет аргументы `plan_set`.
pub fn parse_plan_set(args: &Value) -> Result<PlanSet, PlanError> {
    let list = args["steps"].as_array().ok_or(PlanError::Malformed)?;
    if list.is_empty() {
        return Err(PlanError::Empty);
    }
    // Предел до разбора шагов: огромный список не должен разбираться целиком.
    if list.len() > MAX_STEPS {
        return Err(PlanError::TooMany);
    }
    let mut steps = Vec::with_capacity(list.len());
    for item in list {
        let id = text_of(&item["id"]);
        if id.is_empty() || id.chars().count() > ID_MAX_CHARS {
            return Err(PlanError::BadId);
        }
        let title = clip(&text_of(&item["title"]), TITLE_MAX_CHARS);
        let mut after: Vec<String> = Vec::new();
        for target in item["after"].as_array().into_iter().flatten() {
            let target = text_of(target);
            if !target.is_empty() && !after.contains(&target) {
                // Обрыв сразу, как только ссылок больше предела.
                if after.len() >= MAX_STEPS {
                    return Err(PlanError::Malformed);
                }
                after.push(target);
            }
        }
        steps.push(PlanStep {
            title: if title.is_empty() { id.clone() } else { title },
            id,
            after,
            known: item["known"].as_bool().unwrap_or(false),
        });
    }
    let goal = text_of(&args["goal"]);
    let current = text_of(&args["current"]);
    check_structure(&steps, &goal, &current)?;
    let reason = Some(clip(&text_of(&args["reason"]), REASON_MAX_CHARS)).filter(|r| !r.is_empty());
    Ok(PlanSet {
        steps,
        goal,
        current,
        reason,
    })
}

/// Разбирает аргументы `plan_progress`: наличие шагов проверяется против плана позже.
pub fn parse_plan_progress(args: &Value) -> PlanProgressArgs {
    let current = Some(text_of(&args["current"])).filter(|c| !c.is_empty());
    let mut done: Vec<String> = Vec::new();
    for id in args["done"].as_array().into_iter().flatten() {
        let id = text_of(id);
        if !id.is_empty() && !done.contains(&id) && done.len() < MAX_STEPS {
            done.push(id);
        }
    }
    PlanProgressArgs { current, done }
}

/// Структура плана: уникальные id, ссылки существуют, циклов нет, шагов не больше
/// [`MAX_STEPS`], `goal` и `current` — существующие шаги.
pub fn check_structure(steps: &[PlanStep], goal: &str, current: &str) -> Result<(), PlanError> {
    if steps.is_empty() {
        return Err(PlanError::Empty);
    }
    if steps.len() > MAX_STEPS {
        return Err(PlanError::TooMany);
    }
    let mut ids: HashSet<&str> = HashSet::new();
    for step in steps {
        if !ids.insert(step.id.as_str()) {
            return Err(PlanError::DuplicateId(step.id.clone()));
        }
    }
    for step in steps {
        for target in &step.after {
            if !ids.contains(target.as_str()) {
                return Err(PlanError::UnknownRef {
                    step: clip(&step.id, ID_MAX_CHARS),
                    target: clip(target, ID_MAX_CHARS),
                });
            }
        }
    }
    if !ids.contains(goal) {
        return Err(PlanError::UnknownGoal);
    }
    if !ids.contains(current) {
        return Err(PlanError::UnknownCurrent);
    }
    if layers(steps).is_none() {
        return Err(PlanError::Cycle);
    }
    Ok(())
}

/// Слой каждого шага: длина самого длинного пути от шагов без зависимостей. `None` при
/// цикле (или ссылке на несуществующий шаг).
fn layers(steps: &[PlanStep]) -> Option<Vec<u32>> {
    let index: HashMap<&str, usize> = steps
        .iter()
        .enumerate()
        .map(|(i, step)| (step.id.as_str(), i))
        .collect();
    let mut pending: Vec<usize> = Vec::with_capacity(steps.len());
    let mut users: Vec<Vec<usize>> = vec![Vec::new(); steps.len()];
    for (i, step) in steps.iter().enumerate() {
        pending.push(step.after.len());
        for target in &step.after {
            users[*index.get(target.as_str())?].push(i);
        }
    }
    let mut layer = vec![0u32; steps.len()];
    let mut queue: Vec<usize> = (0..steps.len()).filter(|i| pending[*i] == 0).collect();
    let mut done = 0usize;
    while let Some(i) = queue.pop() {
        done += 1;
        for &user in &users[i] {
            layer[user] = layer[user].max(layer[i] + 1);
            pending[user] -= 1;
            if pending[user] == 0 {
                queue.push(user);
            }
        }
    }
    (done == steps.len()).then_some(layer)
}

/// Клетки шагов `(x, y)` в порядке шагов, число слоёв и число строк.
pub type Layout = (Vec<(u32, u32)>, u32, u32);

/// Раскладка слоями (research.md §4): слой — длина самого длинного пути, порядок в слое —
/// порядок объявления и один проход уменьшения пересечений (по среднему положению
/// зависимостей). Возвращает `(x, y)` каждого шага, число слоёв и строк. Структуру с
/// циклом не раскладывает.
pub fn layout(steps: &[PlanStep]) -> Option<Layout> {
    let layer = layers(steps)?;
    let count = layer.iter().max().map_or(0, |max| *max as usize + 1);
    let index: HashMap<&str, usize> = steps
        .iter()
        .enumerate()
        .map(|(i, step)| (step.id.as_str(), i))
        .collect();
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (i, l) in layer.iter().enumerate() {
        columns[*l as usize].push(i);
    }
    let mut row = vec![0u32; steps.len()];
    for column in &columns {
        for (y, i) in column.iter().enumerate() {
            row[*i] = y as u32;
        }
    }
    // Один проход слева направо: шаг встаёт напротив среднего положения своих зависимостей.
    for column in columns.iter_mut().skip(1) {
        let mut keyed: Vec<(f64, usize)> = column
            .iter()
            .map(|&i| {
                let sum: u32 = steps[i].after.iter().map(|t| row[index[t.as_str()]]).sum();
                (f64::from(sum) / steps[i].after.len().max(1) as f64, i)
            })
            .collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        *column = keyed.into_iter().map(|(_, i)| i).collect();
        for (y, i) in column.iter().enumerate() {
            row[*i] = y as u32;
        }
    }
    let rows = columns.iter().map(Vec::len).max().unwrap_or(0) as u32;
    let places = (0..steps.len()).map(|i| (layer[i], row[i])).collect();
    Some((places, count as u32, rows))
}

/// Номер события, к которому прокручивается лента: ближайшая реплика не позже события
/// плана (она показана в ленте, само событие плана — нет).
fn anchor_of(last_speech: Option<usize>, index: usize) -> u32 {
    last_speech.unwrap_or(index) as u32
}

fn plan_event_ok(steps: &[PlanStep], goal: &str, current: &str) -> bool {
    check_structure(steps, goal, current).is_ok()
}

/// Вид плана по событиям: последняя версия или версия `version` (состояние на момент, когда
/// её сменила следующая). События с ошибкой структуры, например пришедшие с
/// синхронизацией, пропускаются как будто их нет.
pub fn plan_view(events: &[LessonEvent], version: Option<u32>) -> Option<PlanView> {
    let mut count = 0u32;
    let mut latest: Option<(&[PlanStep], &str, Option<&String>)> = None;
    let mut previous: HashSet<&str> = HashSet::new();
    let mut done: HashSet<String> = HashSet::new();
    let mut current: Option<String> = None;
    let mut anchors: HashMap<String, u32> = HashMap::new();
    let mut last_speech: Option<usize> = None;
    for (index, event) in events.iter().enumerate() {
        match event {
            LessonEvent::Teacher { .. } | LessonEvent::User { .. } => last_speech = Some(index),
            LessonEvent::Plan {
                steps,
                goal,
                current: now,
                reason,
                ..
            } if plan_event_ok(steps, goal, now) => {
                if version.is_some_and(|wanted| count >= wanted) {
                    break;
                }
                count += 1;
                previous = latest
                    .map(|(old, _, _)| old.iter().map(|s| s.id.as_str()).collect())
                    .unwrap_or_default();
                latest = Some((steps, goal, reason.as_ref()));
                done.retain(|id| steps.iter().any(|s| &s.id == id));
                if current.as_deref() != Some(now.as_str()) {
                    anchors.insert(now.clone(), anchor_of(last_speech, index));
                    current = Some(now.clone());
                }
            }
            LessonEvent::PlanProgress {
                current: now,
                done: marked,
                ..
            } => {
                let Some((steps, _, _)) = latest else {
                    continue;
                };
                let exists = |id: &String| steps.iter().any(|s| &s.id == id);
                if !exists(now) || !marked.iter().all(exists) {
                    continue;
                }
                done.extend(marked.iter().cloned());
                if current.as_deref() != Some(now.as_str()) {
                    anchors.insert(now.clone(), anchor_of(last_speech, index));
                    current = Some(now.clone());
                }
            }
            _ => {}
        }
    }
    if version.is_some_and(|wanted| count != wanted) {
        return None;
    }
    let (steps, goal, reason) = latest?;
    let current = current?;
    let (places, cols, rows) = layout(steps)?;
    let views = steps
        .iter()
        .zip(&places)
        .map(|(step, (x, y))| PlanStepView {
            id: step.id.clone(),
            title: step.title.clone(),
            after: step.after.clone(),
            known: step.known,
            state: if step.id == current {
                PlanStepState::Current
            } else if done.contains(&step.id) {
                PlanStepState::Done
            } else if step.known {
                PlanStepState::Known
            } else {
                PlanStepState::Todo
            },
            added: count > 1 && !previous.contains(step.id.as_str()),
            anchor: anchors.get(&step.id).copied(),
            x: *x,
            y: *y,
        })
        .collect();
    let edges = steps
        .iter()
        .flat_map(|step| {
            step.after.iter().map(|from| PlanEdge {
                from: from.clone(),
                to: step.id.clone(),
            })
        })
        .collect();
    Some(PlanView {
        version: count,
        goal: goal.to_owned(),
        current,
        steps: views,
        edges,
        cols,
        rows,
        changes: count - 1,
        last_reason: reason.cloned(),
    })
}

/// Принятые версии плана от первой.
pub fn plan_history(events: &[LessonEvent]) -> Vec<PlanVersionInfo> {
    let mut history = Vec::new();
    for event in events {
        if let LessonEvent::Plan {
            at,
            steps,
            goal,
            current,
            reason,
            ..
        } = event
        {
            if plan_event_ok(steps, goal, current) {
                history.push(PlanVersionInfo {
                    version: history.len() as u32 + 1,
                    at: *at,
                    steps: steps.len() as u32,
                    reason: reason.clone(),
                });
            }
        }
    }
    history
}

/// Проверка `plan_progress` против последней версии плана.
pub fn check_progress(events: &[LessonEvent], args: &PlanProgressArgs) -> Result<(), PlanError> {
    let view = plan_view(events, None).ok_or(PlanError::NoPlan)?;
    let exists = |id: &String| view.steps.iter().any(|s| &s.id == id);
    if args.current.as_ref().is_some_and(|c| !exists(c)) {
        return Err(PlanError::UnknownCurrent);
    }
    if let Some(missing) = args.done.iter().find(|id| !exists(id)) {
        return Err(PlanError::UnknownRef {
            step: "done".to_owned(),
            target: clip(missing, ID_MAX_CHARS),
        });
    }
    Ok(())
}
