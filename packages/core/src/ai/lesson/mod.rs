//! Сессия урока с ИИ-учителем (фича 051): файл урока, ход учителя с инструментами,
//! потоковый разбор ответа, сжатие длинной переписки и восстановление оборванного
//! ответа. Модуль строится на запросах 050: журнал отправленного, проверка доступа,
//! отмена, ключи. Отказы — `AiFailure` без текста.

pub mod file;
pub mod partial;
pub mod plan;
pub mod prompt;
pub mod stream;
pub mod turn;
#[cfg(target_arch = "wasm32")]
pub mod web;

use std::path::Path;

pub use file::{
    Confidence, Language, LessonEvent, LessonHeader, LessonPhase, LessonQuestion, LessonState,
    LessonSummary, LessonView, NoteOutcome, ProbePart, ProbeState, ProbeSummary, Waiting,
};
pub use partial::LessonProgress;
pub use plan::{PlanEdge, PlanStep, PlanStepState, PlanStepView, PlanVersionInfo, PlanView};
pub use stream::{ToolCall, TurnReply};
pub use turn::{Next, PreparedTurn, TurnDone, TurnKind, UserAnswer};

use super::access::normalize_folder;
use super::settings::load_settings;
use super::AiFailure;
use crate::vault::backend::{ReadError, VaultBackend};
use crate::vault::{classify, FileKind, LESSON_EXTENSION};
use file::{
    append_events, initial_text, parse, read_lesson, summary_of, view_of, FORMAT, GOAL_MAX_CHARS,
};

/// Символ категории Cf (управление направлением письма, невидимые разделители): в имени
/// файла он подменил бы то, что видит пользователь (ревью безопасности 051, L7).
fn is_format_char(c: char) -> bool {
    matches!(c as u32,
        0x00AD | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F | 0x08E2 | 0x180E
        | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F
        | 0xFEFF | 0xFFF9..=0xFFFB | 0x110BD | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A
        | 0xE0001 | 0xE0020..=0xE007F)
}

/// Имена устройств Windows: файл с таким именем (с любым расширением) там не создать.
fn is_reserved_name(name: &str) -> bool {
    let base = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.'])
        .to_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || ["COM", "LPT"].iter().any(|prefix| {
            base.strip_prefix(prefix).is_some_and(|rest| {
                matches!(rest.chars().collect::<Vec<_>>().as_slice(), [d] if d.is_ascii_digit() || matches!(d, '¹' | '²' | '³'))
            })
        })
}

/// Имя файла из цели: без символов, которых нет в именах файлов, не длиннее 40 символов.
fn file_stem(goal: &str) -> String {
    let first_line = goal.lines().next().unwrap_or_default();
    let cleaned: String = first_line
        .chars()
        .map(|c| {
            if c.is_control() || is_format_char(c) || "/\\:*?\"<>|".contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    let joined: String = words.join(" ").chars().take(40).collect();
    let trimmed = joined.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        "Урок".to_owned()
    } else if is_reserved_name(trimmed) {
        format!("_{trimmed}")
    } else {
        trimmed.to_owned()
    }
}

/// Сколько заметок можно прикрепить к уроку при создании.
pub const MAX_ATTACHED_NOTES: usize = 10;

/// Создаёт урок: файл в папке уроков, прикреплённые заметки — событиями `NoteRead`
/// (FR-011). Заметка из запрещённой папки или вне vault прикрепляется отказом, а не
/// содержимым. Выключенное обучение отказывает, не создавая файла (FR-001). Прикрепляется
/// не больше [`MAX_ATTACHED_NOTES`] заметок; пути сравниваются после нормализации.
pub async fn create_lesson<B: VaultBackend>(
    backend: &mut B,
    goal: &str,
    notes: &[String],
    language: Language,
    start: LessonPhase,
    now: i64,
) -> Result<String, AiFailure> {
    let settings = load_settings(backend).await;
    if !settings.enabled {
        return Err(AiFailure::Disabled);
    }
    let goal = goal.trim();
    if goal.is_empty() {
        return Err(AiFailure::GoalEmpty);
    }
    let goal: String = goal.chars().take(GOAL_MAX_CHARS).collect();

    let mut events: Vec<LessonEvent> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for path in notes {
        // Путь, который не привести к чистому виду, остаётся как есть и получает отказ.
        let path = normalize_folder(Path::new(path)).unwrap_or_else(|| path.clone());
        if seen.contains(&path) {
            continue;
        }
        if seen.len() >= MAX_ATTACHED_NOTES {
            break;
        }
        seen.push(path.clone());
        let (outcome, _) = turn::read_attached(backend, &settings, &path).await;
        events.push(LessonEvent::NoteRead {
            at: now,
            path,
            by_user: true,
            outcome,
        });
    }
    let header = LessonHeader {
        format: FORMAT,
        goal: goal.clone(),
        language,
        created_at: now,
        start,
    };
    let text = initial_text(&header, &events)?;

    let folder = Path::new(&settings.lessons_folder);
    // Ссылка на полпути увела бы папку и файл за пределы vault: проверка до создания.
    if backend.crosses_symlink(&folder.join("_")).await || backend.is_symlink(folder).await {
        return Err(AiFailure::JournalFailed);
    }
    backend
        .ensure_folder(folder)
        .await
        .map_err(|_| AiFailure::JournalFailed)?;
    let stem = file_stem(&goal);
    let mut number = 1u32;
    let path = loop {
        let name = if number == 1 {
            format!("{stem}.{LESSON_EXTENSION}")
        } else {
            format!("{stem} {number}.{LESSON_EXTENSION}")
        };
        let candidate = folder.join(name);
        // Занято всё, что существует, а не только то, что читается: ссылка (в том числе
        // висячая), файл без прав и слишком большой файл имя не освобождают (M1).
        let free = !backend.is_symlink(&candidate).await
            && matches!(
                backend.read_bytes_any_size(&candidate).await,
                Err(ReadError::NotFound)
            );
        if free {
            break candidate;
        }
        number += 1;
        if number > 10_000 {
            return Err(AiFailure::JournalFailed);
        }
    };
    backend
        .write(&path, &text, None)
        .await
        .map_err(|_| AiFailure::JournalFailed)?;
    Ok(path.to_string_lossy().replace('\\', "/"))
}

/// Открывает урок. Если в прошлый раз приложение закрыли посреди ответа, полученное из
/// промежуточного файла дописывается оборванной репликой (research.md §5, SC-002).
pub async fn open_lesson<B: VaultBackend>(
    backend: &mut B,
    path: &str,
) -> Result<LessonView, AiFailure> {
    let mut parsed = read_lesson(backend, path).await?;
    if parsed.header.format <= FORMAT && !partial::is_active(path) {
        if let Some(text) = partial::read_partial(backend, path).await {
            if matches!(
                parsed.events.last(),
                Some(LessonEvent::User { .. } | LessonEvent::NoteRead { .. })
            ) {
                let at = parsed.events.last().map_or(0, LessonEvent::at);
                let model = load_settings(backend).await.model.unwrap_or_default();
                let event = LessonEvent::Teacher {
                    at,
                    text,
                    question: None,
                    interrupted: true,
                    failure: None,
                    model,
                    input_tokens: None,
                    output_tokens: None,
                    cost_usd: None,
                };
                append_events(backend, path, std::slice::from_ref(&event)).await?;
                parsed.events.push(event);
            }
            partial::clear_partial(backend, path).await;
        }
    }
    Ok(view_of(path, parsed))
}

/// Все уроки vault, не только из папки уроков (research.md §9); свежие первыми.
/// Нечитаемые файлы в список не попадают.
pub async fn list_lessons<B: VaultBackend>(backend: &B) -> Vec<LessonSummary> {
    let mut lessons = Vec::new();
    for note in backend.list().await {
        if classify(&note.path) != FileKind::Lesson {
            continue;
        }
        let path = note.path.to_string_lossy().replace('\\', "/");
        let Ok(bytes) = backend.read_bytes(&note.path).await else {
            continue;
        };
        if let Ok(parsed) = parse(&bytes) {
            lessons.push(summary_of(&path, &parsed));
        }
    }
    lessons.sort_by(|a, b| b.last_at.cmp(&a.last_at).then_with(|| a.path.cmp(&b.path)));
    lessons
}

async fn append_and_view<B: VaultBackend>(
    backend: &mut B,
    path: &str,
    event: LessonEvent,
) -> Result<LessonView, AiFailure> {
    let parsed = read_lesson(backend, path).await?;
    if parsed.header.format > FORMAT {
        return Err(AiFailure::LessonUnreadable);
    }
    append_events(backend, path, std::slice::from_ref(&event)).await?;
    open_lesson(backend, path).await
}

/// Завершает, останавливает или возобновляет урок.
pub async fn set_state<B: VaultBackend>(
    backend: &mut B,
    path: &str,
    state: LessonState,
    now: i64,
) -> Result<LessonView, AiFailure> {
    append_and_view(backend, path, LessonEvent::State { at: now, state }).await
}

pub async fn set_language<B: VaultBackend>(
    backend: &mut B,
    path: &str,
    language: Language,
    now: i64,
) -> Result<LessonView, AiFailure> {
    append_and_view(backend, path, LessonEvent::Language { at: now, language }).await
}

/// Пропускает замер: событие `Phase { Teach, skipped }`, дальше учитель ведёт урок без
/// итога замера. Урок уже в стадии обучения остаётся как есть (повторный вызов безвреден).
pub async fn skip_probe<B: VaultBackend>(
    backend: &mut B,
    path: &str,
    now: i64,
) -> Result<LessonView, AiFailure> {
    let parsed = read_lesson(backend, path).await?;
    if parsed.header.format > FORMAT {
        return Err(AiFailure::LessonUnreadable);
    }
    if file::current_phase(&parsed.header, &parsed.events) != LessonPhase::Probe {
        return open_lesson(backend, path).await;
    }
    let event = LessonEvent::Phase {
        at: now,
        phase: LessonPhase::Teach,
        skipped: true,
    };
    append_events(backend, path, std::slice::from_ref(&event)).await?;
    open_lesson(backend, path).await
}

/// План версии `version` с раскладкой; `None`, если такой версии нет (research.md §3).
pub async fn plan_version<B: VaultBackend>(
    backend: &B,
    path: &str,
    version: u32,
) -> Result<Option<PlanView>, AiFailure> {
    let parsed = read_lesson(backend, path).await?;
    Ok(plan::plan_view(&parsed.events, Some(version)))
}
