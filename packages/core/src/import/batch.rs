//! Пакет импорта: обзор, план, шаги, журнал и итог (фича 021, research.md §7, §8).
//!
//! По образцу пакетного экспорта 009 (`export::batch`), но с одним отличием:
//! состояние плана — тысячи файлов и карта имён — между шагами не
//! пересылается. Оболочка носит только [`ImportState`] со счётчиками, а сам
//! план лежит в ядре под идентификатором пакета (Decision в research.md §7:
//! иначе шаг стоил бы сериализации всего плана, а 2 000 шагов — сотен
//! мегабайт). Потерянный план (перезагрузка страницы) не страшен: новый
//! `import_begin` читает журнал и продолжает без дублей.
//!
//! # Гарантии
//!
//! * Отмена действует между шагами; шаг — один файл, и запись атомарна
//!   ([`VaultBackend::write_bytes`]), поэтому полузаписанных файлов нет
//!   (принцип X, FR-010).
//! * Существующий файл назначения не перезаписывается (FR-009).
//! * Источник только читается: он приходит как [`ReadOnly`] (FR-002).
//! * Заметка пишется после своих вложений: ссылки не повисают в воздухе.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::markdown::convert_with;
use super::names::{
    journal_id, Attachments, NoteResolver, Plan, PlannedFile, SourceKind, BESIDE_SUFFIX,
};
use super::report::{self, Totals};
use super::{
    ImportError, ImportJob, ImportJournal, ImportPhase, ImportState, ImportSurvey, JournalEntry,
    OnConflict, Outcome, ReportEntry, ResumeInfo, SkipReason, JOURNAL_VERSION,
};
use crate::boards::placement::AssetsPlacement;
use crate::vault::backend::{ReadError, ReadOnly, VaultBackend, WriteError};
use crate::vault::sections::SectionMark;
use crate::vault::service_dir::service_dir;
use crate::vault::vault_settings::read_vault_settings;

/// Сколько шагов между записями журнала. Запись журнала целиком на каждом шаге
/// стоила бы квадратичного объёма записи; цена редкой записи — при внезапном
/// обрыве до этого числа заметок будут переписаны (research.md §8).
const JOURNAL_EVERY: u32 = 16;

/// Сколько последних проблемных файлов хранит состояние для карточки хода.
const RECENT: usize = 20;

/// Сколько раз заметка пересобирается, если вложение не скопировалось.
const MAX_REBUILDS: usize = 3;

/// Всё, что шагам нужно знать о запуске.
struct Run {
    /// Идентификатор журнала (хэш источника и назначения).
    journal_id: String,
    plan: Plan,
    attachments: Attachments,
    unavailable: HashSet<String>,
    job: ImportJob,
    source_hint: String,
    destination: String,
    placement: AssetsPlacement,
    sections: Vec<SectionMark>,
    /// Файлы с потерями, пропущенные и переименованные.
    report: Vec<ReportEntry>,
    totals: Totals,
    journal: ImportJournal,
    unsaved_steps: u32,
}

/// Планы активных пакетов. Глобальное состояние осознанное: см. заголовок
/// модуля. Шаг забирает запуск из таблицы на время работы и кладёт обратно, так
/// что замок не держится через `await`.
static RUNS: Mutex<BTreeMap<String, Run>> = Mutex::new(BTreeMap::new());

/// Счётчик запусков: два пакета с одним источником и назначением (две вкладки,
/// параллельные тесты) не должны делить план.
static RUN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn take_run(id: &str) -> Option<Run> {
    RUNS.lock().ok()?.remove(id)
}

/// Сколько незавершённых пакетов ядро помнит: брошенный без `import_finish` план
/// не должен копиться до конца сеанса.
const MAX_RUNS: usize = 64;

fn put_run(id: String, run: Run) {
    if let Ok(mut runs) = RUNS.lock() {
        while runs.len() >= MAX_RUNS {
            let Some(oldest) = runs.keys().next().cloned() else {
                break;
            };
            runs.remove(&oldest);
        }
        runs.insert(id, run);
    }
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn journal_path(id: &str) -> PathBuf {
    service_dir()
        .join(crate::vault::service_dir::IMPORTS_DIR_NAME)
        .join(format!("{id}.json"))
}

/// Общая подготовка обзора и запуска.
struct Prepared {
    files: Vec<(String, u64)>,
    excluded: u32,
    existing: HashSet<String>,
    journal: Option<ImportJournal>,
    id: String,
    hint: String,
    destination: String,
    /// Перенесённое прошлым запуском, назначение которого ещё лежит в vault.
    done: HashMap<String, String>,
    /// Шагов, которые прошлый запуск довёл до конца: перенесённые, вложения и
    /// окончательно пропущенные.
    processed: u32,
}

async fn prepare<S: VaultBackend, V: VaultBackend>(
    source: &ReadOnly<S>,
    vault: &V,
    job: &ImportJob,
) -> Prepared {
    let listing = source.list_source_files().await;
    let files: Vec<(String, u64)> = listing
        .files
        .iter()
        .map(|file| (slash(&file.path), file.size))
        .collect();
    let existing: HashSet<String> = vault
        .list_source_files()
        .await
        .files
        .iter()
        .map(|file| slash(&file.path).to_lowercase())
        .collect();
    let destination = job.destination.trim_matches('/').to_owned();
    let hint = slash(source.root_path());
    let id = journal_id(&hint, &destination);

    let journal = match vault.read(&journal_path(&id)).await {
        Ok(text) => serde_json::from_str::<ImportJournal>(&text)
            .ok()
            .filter(|journal| {
                journal.version == JOURNAL_VERSION && journal.destination == destination
            }),
        Err(_) => None,
    };
    let known: HashSet<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
    let done: HashMap<String, String> = journal
        .as_ref()
        .map(|journal| {
            journal
                .done
                .iter()
                .filter(|entry| {
                    known.contains(entry.source.as_str())
                        && existing.contains(&entry.destination.to_lowercase())
                })
                .map(|entry| (entry.source.clone(), entry.destination.clone()))
                .collect()
        })
        .unwrap_or_default();
    let processed = journal
        .as_ref()
        .map(|journal| {
            let attachments = journal
                .attachments
                .iter()
                .filter(|entry| {
                    known.contains(entry.source.as_str())
                        && existing.contains(&entry.destination.to_lowercase())
                })
                .count();
            let skipped = journal
                .skipped
                .iter()
                .filter(|path| known.contains(path.as_str()))
                .count();
            (done.len() + attachments + skipped) as u32
        })
        .unwrap_or(0);
    Prepared {
        files,
        excluded: listing.excluded,
        processed,
        existing,
        journal,
        id,
        hint,
        destination,
        done,
    }
}

/// Обзор источника (экран C1) и возможность продолжения (C4).
pub async fn survey<S: VaultBackend, V: VaultBackend>(
    source: &ReadOnly<S>,
    vault: &V,
    job: &ImportJob,
) -> ImportSurvey {
    let prepared = prepare(source, vault, job).await;
    let (mut notes, mut images, mut other) = (0, 0, 0);
    for (path, _) in &prepared.files {
        match super::names::kind_of(path) {
            SourceKind::Note => notes += 1,
            SourceKind::Image => images += 1,
            SourceKind::Other => other += 1,
        }
    }
    let total = prepared.files.len() as u32;
    let done = prepared.processed;
    let resume = prepared
        .journal
        .as_ref()
        .filter(|_| done > 0 && done < total)
        .map(|journal| ResumeInfo {
            done,
            total,
            at: journal.started_ms,
        });
    ImportSurvey {
        notes,
        images,
        other,
        excluded: prepared.excluded,
        resume,
    }
}

/// Строит план и запускает пакет. Ничего не пишет.
pub async fn import_begin<S: VaultBackend, V: VaultBackend>(
    source: &ReadOnly<S>,
    vault: &V,
    job: &ImportJob,
) -> ImportState {
    let prepared = prepare(source, vault, job).await;
    let plan = Plan::build(&prepared.files, job, &prepared.existing, &prepared.done);
    let settings = read_vault_settings(vault).await.unwrap_or_default();

    let mut attachments = Attachments::new(plan.taken.clone());
    if let Some(journal) = &prepared.journal {
        for entry in &journal.attachments {
            if prepared
                .existing
                .contains(&entry.destination.to_lowercase())
            {
                attachments
                    .claimed
                    .insert(entry.source.clone(), entry.destination.clone());
                attachments.taken.insert(entry.destination.to_lowercase());
            }
        }
    }
    let journal = ImportJournal {
        version: JOURNAL_VERSION,
        source_hint: prepared.hint.clone(),
        destination: prepared.destination.clone(),
        started_ms: job.started_ms,
        total: plan.items.len() as u32,
        done: prepared
            .done
            .iter()
            .map(|(source, destination)| JournalEntry {
                source: source.clone(),
                destination: destination.clone(),
            })
            .collect(),
        attachments: attachments
            .claimed
            .iter()
            .map(|(source, destination)| JournalEntry {
                source: source.clone(),
                destination: destination.clone(),
            })
            .collect(),
        skipped: prepared
            .journal
            .as_ref()
            .map(|journal| journal.skipped.clone())
            .unwrap_or_default(),
    };
    let total = plan.items.len() as u32;
    let key = format!(
        "{}-{}",
        prepared.id,
        RUN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let state = ImportState {
        id: key.clone(),
        destination: prepared.destination.clone(),
        phase: if total == 0 {
            ImportPhase::Done
        } else {
            ImportPhase::Running
        },
        next: 0,
        total,
        clean: 0,
        lossy: 0,
        skipped: 0,
        resumed: 0,
        current: None,
        recent: Vec::new(),
    };
    put_run(
        key,
        Run {
            journal_id: prepared.id,
            plan,
            attachments,
            unavailable: HashSet::new(),
            job: job.clone(),
            source_hint: prepared.hint,
            destination: prepared.destination,
            placement: settings.placement(),
            sections: settings.sections.clone(),
            report: Vec::new(),
            totals: Totals::default(),
            journal,
            unsaved_steps: 0,
        },
    );
    state
}

/// Один шаг: один файл источника. Отмена — просто не звать следующий шаг.
pub async fn import_step<S: VaultBackend, V: VaultBackend>(
    source: &ReadOnly<S>,
    vault: &mut V,
    mut state: ImportState,
) -> Result<ImportState, ImportError> {
    if state.next >= state.total {
        state.phase = ImportPhase::Done;
        return Ok(state);
    }
    let mut run = take_run(&state.id).ok_or(ImportError::SessionLost)?;
    let item = run.plan.items[state.next as usize].clone();

    let entry = process(&mut run, source, vault, &item).await;
    state.current = Some(item.source.clone());
    state.next += 1;
    match entry {
        None => {
            // Перенесён прошлым запуском или общая картинка, уже скопированная
            // заметкой: шаг учтён, строки в отчёте нет.
            if item.done {
                state.resumed += 1;
                run.totals.resumed += 1;
            } else {
                state.clean += 1;
                run.totals.clean += 1;
            }
        }
        Some(entry) => {
            match entry.outcome {
                Outcome::Clean => {
                    state.clean += 1;
                    run.totals.clean += 1;
                }
                Outcome::Lossy => {
                    state.lossy += 1;
                    run.totals.lossy += 1;
                }
                Outcome::Skipped => {
                    state.skipped += 1;
                    run.totals.skipped += 1;
                }
            }
            if entry.outcome == Outcome::Skipped
                && matches!(
                    entry.skip_reason,
                    Some(
                        SkipReason::TooLarge
                            | SkipReason::NotUtf8
                            | SkipReason::OtherFile
                            | SkipReason::ExistsSkipped
                    )
                )
                && !run.journal.skipped.contains(&entry.source)
            {
                run.journal.skipped.push(entry.source.clone());
            }
            if entry.outcome != Outcome::Clean || entry.renamed.is_some() {
                if entry.outcome != Outcome::Clean {
                    state.recent.push(entry.clone());
                    if state.recent.len() > RECENT {
                        state.recent.remove(0);
                    }
                }
                run.report.push(entry);
            }
        }
    }
    if state.next >= state.total {
        state.phase = ImportPhase::Done;
    }

    run.unsaved_steps += 1;
    if run.unsaved_steps >= JOURNAL_EVERY || state.phase == ImportPhase::Done {
        flush_journal(vault, &mut run).await;
    }
    put_run(state.id.clone(), run);
    Ok(state)
}

/// Заканчивает пакет — и после последнего шага, и после отмены: записывает
/// журнал и заметку отчёта, снимает план. Возвращает путь заметки отчёта.
pub async fn import_finish<V: VaultBackend>(
    vault: &mut V,
    state: &ImportState,
) -> Result<String, ImportError> {
    let mut run = take_run(&state.id).ok_or(ImportError::SessionLost)?;
    flush_journal(vault, &mut run).await;

    let cancelled = state.next < state.total;
    let texts = run.job.texts.clone();
    let name = super::names::sanitize_component(&texts.file_name);
    let root = run.destination.clone();
    let build = |name: &str| {
        if root.is_empty() {
            format!("{name}.typ")
        } else {
            format!("{root}/{name}.typ")
        }
    };
    let mut path = build(&name);
    let mut n = 1;
    while vault.file_mark(Path::new(&path)).await.is_some() {
        n += 1;
        path = build(&format!("{name} ({n})"));
    }
    let text = report::build(
        &texts,
        &run.source_hint,
        &run.destination,
        &path,
        &run.report,
        run.totals,
        cancelled,
    );
    write_file(vault, &path, text.as_bytes())
        .await
        .map_err(|error| match error {
            WriteError::AccessDenied => ImportError::AccessDenied,
            _ => ImportError::Io,
        })?;
    Ok(path)
}

async fn flush_journal<V: VaultBackend>(vault: &mut V, run: &mut Run) {
    run.unsaved_steps = 0;
    run.journal.attachments = run
        .attachments
        .claimed
        .iter()
        .filter(|(source, _)| !run.attachments.pending.iter().any(|(s, _)| s == *source))
        .map(|(source, destination)| JournalEntry {
            source: source.clone(),
            destination: destination.clone(),
        })
        .collect();
    run.journal
        .attachments
        .sort_by(|a, b| a.source.cmp(&b.source));
    if let Ok(text) = serde_json::to_string(&run.journal) {
        // Отказ записи журнала импорт не останавливает: потеряется только
        // возможность продолжить без повторной работы.
        let _ = write_file(
            vault,
            &slash(&journal_path(&run.journal_id)),
            text.as_bytes(),
        )
        .await;
    }
}

// ── Один файл ────────────────────────────────────────────────────────────────

async fn write_file<V: VaultBackend>(
    vault: &mut V,
    path: &str,
    bytes: &[u8],
) -> Result<(), WriteError> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            vault.ensure_folder(parent).await?;
        }
    }
    vault.write_bytes(Path::new(path), bytes).await
}

fn skipped(item: &PlannedFile, reason: SkipReason) -> ReportEntry {
    ReportEntry {
        source: item.source.clone(),
        destination: None,
        outcome: Outcome::Skipped,
        losses: Vec::new(),
        skip_reason: Some(reason),
        renamed: None,
    }
}

async fn process<S: VaultBackend, V: VaultBackend>(
    run: &mut Run,
    source: &ReadOnly<S>,
    vault: &mut V,
    item: &PlannedFile,
) -> Option<ReportEntry> {
    if item.done {
        return None;
    }
    match item.kind {
        SourceKind::Note => Some(import_note(run, source, vault, item).await),
        SourceKind::Image if run.attachments.claimed.contains_key(&item.source) => None,
        SourceKind::Image | SourceKind::Other => Some(import_other(run, source, vault, item).await),
    }
}

async fn import_note<S: VaultBackend, V: VaultBackend>(
    run: &mut Run,
    source: &ReadOnly<S>,
    vault: &mut V,
    item: &PlannedFile,
) -> ReportEntry {
    if let Some(reason) = item.skip {
        return skipped(item, reason);
    }
    let bytes = match source.read_bytes_any_size(Path::new(&item.source)).await {
        Ok(bytes) => bytes,
        Err(ReadError::TooLarge) => return skipped(item, SkipReason::TooLarge),
        Err(_) => return skipped(item, SkipReason::Unreadable),
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return skipped(item, SkipReason::NotUtf8);
    };
    // Файл мог появиться после построения плана: чужое не перезаписываем.
    if vault
        .file_mark(Path::new(&item.destination))
        .await
        .is_some()
    {
        return skipped(item, SkipReason::ExistsSkipped);
    }

    let mut converted = None;
    for _ in 0..MAX_REBUILDS {
        let result = {
            let mut resolver = NoteResolver {
                plan: &run.plan,
                attachments: &mut run.attachments,
                source: &item.source,
                destination: &item.destination,
                placement: run.placement,
                sections: &run.sections,
                unavailable: &run.unavailable,
            };
            convert_with(&text, &mut resolver)
        };
        let pending = std::mem::take(&mut run.attachments.pending);
        let mut failed = false;
        for (from, to) in pending {
            if copy_attachment(run, source, vault, &from, &to).await {
                run.journal.attachments.push(JournalEntry {
                    source: from,
                    destination: to,
                });
            } else {
                // Не скопировалось: картинка считается ненайденной, заметка
                // собирается заново.
                run.attachments.claimed.remove(&from);
                run.unavailable.insert(from);
                failed = true;
            }
        }
        converted = Some(result);
        if !failed {
            break;
        }
    }
    let converted = converted.expect("хотя бы одна сборка выполнена");

    if let Err(error) = write_file(vault, &item.destination, converted.typst.as_bytes()).await {
        let _ = error;
        return skipped(item, SkipReason::WriteFailed);
    }
    run.journal.done.push(JournalEntry {
        source: item.source.clone(),
        destination: item.destination.clone(),
    });
    let mut losses = converted.losses;
    // Ссылка на раздел и неоднозначность — потери только в смысле отчёта.
    losses.sort_by_key(|loss| (loss.line, loss.kind as u8));
    let outcome = if losses.is_empty() {
        Outcome::Clean
    } else {
        Outcome::Lossy
    };
    ReportEntry {
        source: item.source.clone(),
        destination: Some(item.destination.clone()),
        outcome,
        losses,
        skip_reason: None,
        renamed: item.renamed.clone(),
    }
}

/// Копирует вложение, если его ещё нет в назначении. `true` — файл на месте.
async fn copy_attachment<S: VaultBackend, V: VaultBackend>(
    _run: &Run,
    source: &ReadOnly<S>,
    vault: &mut V,
    from: &str,
    to: &str,
) -> bool {
    let Ok(bytes) = source.read_bytes_any_size(Path::new(from)).await else {
        return false;
    };
    if vault.file_mark(Path::new(to)).await.is_some() {
        // Имя занято после построения плана: не перезаписываем и не ссылаемся.
        return false;
    }
    write_file(vault, to, &bytes).await.is_ok()
}

async fn import_other<S: VaultBackend, V: VaultBackend>(
    run: &mut Run,
    source: &ReadOnly<S>,
    vault: &mut V,
    item: &PlannedFile,
) -> ReportEntry {
    if let Some(reason) = item.skip {
        return skipped(item, reason);
    }
    let bytes = match source.read_bytes_any_size(Path::new(&item.source)).await {
        Ok(bytes) => bytes,
        Err(_) => return skipped(item, SkipReason::Unreadable),
    };
    let mut destination = item.destination.clone();
    let mut renamed = item.renamed.clone();
    if vault.file_mark(Path::new(&destination)).await.is_some() {
        if run.job.on_conflict == OnConflict::Skip {
            return skipped(item, SkipReason::ExistsSkipped);
        }
        let (stem, extension) = match destination.rsplit_once('.') {
            Some((stem, extension)) if !stem.ends_with('/') && !extension.contains('/') => {
                (stem.to_owned(), format!(".{extension}"))
            }
            _ => (destination.clone(), String::new()),
        };
        let mut n = 1;
        loop {
            let tag = if n == 1 {
                format!("{stem} ({BESIDE_SUFFIX}){extension}")
            } else {
                format!("{stem} ({BESIDE_SUFFIX} {n}){extension}")
            };
            if vault.file_mark(Path::new(&tag)).await.is_none() {
                destination = tag;
                break;
            }
            n += 1;
        }
        renamed = Some(destination.clone());
    }
    if write_file(vault, &destination, &bytes).await.is_err() {
        return skipped(item, SkipReason::WriteFailed);
    }
    run.journal.done.push(JournalEntry {
        source: item.source.clone(),
        destination: destination.clone(),
    });
    ReportEntry {
        source: item.source.clone(),
        destination: Some(destination),
        outcome: Outcome::Clean,
        losses: Vec::new(),
        skip_reason: None,
        renamed,
    }
}
