use crate::vault::conflict::{DiffLine, EditConflict};
use crate::vault::events::VaultEvent;
use crate::vault::index::{FileStamp, Note};
use crate::vault::relocate::{
    MoveControl, MoveError, MoveImpact, MoveOutcome, MoveRefusal, MoveRequest, MoveTarget,
};
use crate::vault::AccessState;
use std::path::{Path, PathBuf};

/// Проверка имени файла заметки на допустимость для файловой системы (FR-013):
/// непустое, без разделителей пути (`/`, `\`) и без NUL-байта. Общая для
/// `NativeBackend` и `WebBackend` (принцип II, NON-NEGOTIABLE) — оба backend
/// MUST отклонять один и тот же набор имён, а не расходиться по платформе.
pub fn is_valid_note_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', '\0'])
}

/// Извлекает и валидирует конечный компонент имени из пути операции
/// (`create`'s `path`, `rename`'s `to`) — `None`, если путь не заканчивается
/// допустимым именем (пустой путь, спецсимволы и т.п.). Вызывается ДО любого
/// обращения к диску (FR-013: «...до внесения каких-либо изменений на диск»).
pub fn valid_note_name(path: &Path) -> Option<&str> {
    path.file_name()
        .and_then(|n| n.to_str())
        .filter(|n| is_valid_note_name(n))
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum OpenError {
    #[error("папка не найдена")]
    NotFound,
    #[error("нет прав доступа к папке")]
    AccessDenied,
    #[error("путь не является папкой")]
    NotADirectory,
    /// FR-038: заметок больше [`crate::vault::SUPPORTED_NOTE_COUNT`]. Отдельный
    /// вариант, а не `Io`: пользователю MUST быть названа именно эта причина,
    /// а приложение MUST остаться работоспособным — это отказ открыть
    /// конкретную папку, а не сбой.
    #[error("vault превышает поддерживаемый размер")]
    TooManyNotes,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ReadError {
    #[error("заметка не найдена")]
    NotFound,
    #[error("не удалось прочитать содержимое")]
    Io,
    /// Заметка больше [`crate::vault::NOTE_SIZE_LIMIT_BYTES`] (spec Assumptions,
    /// Edge Cases). Сама заметка остаётся в индексе и видна в дереве — отказ
    /// касается только открытия, и причина обязана быть названа.
    #[error("заметка превышает поддерживаемый размер")]
    TooLarge,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum WriteError {
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("версия на диске разошлась с ожидаемой")]
    Conflict,
    #[error("не удалось записать на диск")]
    Io,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum CreateError {
    #[error("имя уже занято в этой папке")]
    NameConflict,
    #[error("имя недопустимо для файловой системы")]
    InvalidName,
    /// FR-036: права на запись отозваны. Отдельный вариант, а не `Io`, —
    /// причина отказа обязана быть названа пользователем, а «не удалось
    /// создать файл» причиной не является.
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("не удалось создать файл")]
    Io,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum RenameError {
    #[error("имя уже занято в этой папке")]
    NameConflict,
    #[error("имя недопустимо для файловой системы")]
    InvalidName,
    #[error("заметка не найдена")]
    NotFound,
    /// FR-036 (права на запись отозваны) и FR-020 (vault недоступен —
    /// переименование запрещено явно, как разрушительное действие).
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("не удалось переименовать файл")]
    Io,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum DeleteError {
    #[error("заметка не найдена")]
    NotFound,
    /// FR-036 (права на запись отозваны) и FR-020 (vault недоступен —
    /// удаление запрещено явно, как разрушительное действие).
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("не удалось удалить файл")]
    Io,
}

/// Отказ разрешения конфликта редактирования (FR-022, FR-023). Отдельный от
/// `WriteError` тип: «разрешать нечего» — не ошибка записи, а рассогласование
/// UI с ядром (диалог показан по конфликту, которого уже нет), и UI обязан
/// отличать её от отказа диска.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ResolveError {
    #[error("для этой заметки нет неразрешённого конфликта")]
    NoPendingConflict,
    #[error("нет прав на запись")]
    AccessDenied,
    #[error("не удалось прочитать или записать заметку")]
    Io,
}

impl From<WriteError> for ResolveError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::AccessDenied => ResolveError::AccessDenied,
            // `Conflict` здесь недостижим: разрешение конфликта пишет без
            // `expected`-снимка — пользователь уже сделал явный выбор.
            WriteError::Conflict | WriteError::Io => ResolveError::Io,
        }
    }
}

/// Единственная точка входа в vault-логику (принцип II конституции,
/// NON-NEGOTIABLE): один трейт, две реализации (`NativeBackend`, `WebBackend`) за
/// одной абстракцией. Контракт — contracts/vault-backend-trait.md.
///
/// `async fn` в трейте — осознанный выбор: у нас нет `dyn VaultBackend` (каждый
/// таргет выбирает конкретный тип через cfg на этапе компиляции), поэтому
/// отсутствие явных auto-trait bounds на будущем не является проблемой.
#[allow(async_fn_in_trait)]
pub trait VaultBackend: Sized {
    async fn open(root_path: &Path) -> Result<Self, OpenError>;

    fn root_path(&self) -> &Path;
    fn access_state(&self) -> AccessState;

    /// Существовало ли служебное место (`.typsidian/`) **в момент открытия**
    /// vault (фича 006, FR-024).
    ///
    /// Спрашивается именно про момент открытия, потому что открытие само это
    /// место и создаёт: спросить позже — значит всегда получить `true`.
    ///
    /// Нужно ровно для одного решения: отличить «пользователь удалил
    /// служебное место вместе с настройками» от «эту папку открывают впервые».
    /// Само решение принимает
    /// [`crate::vault::vault_settings::service_dir_status`], потому что для
    /// него нужен ещё и факт из настроек устройства, о котором backend не
    /// знает и знать не должен.
    fn service_dir_existed_at_open(&self) -> bool;

    async fn list(&self) -> Vec<Note>;
    async fn list_folder(&self, folder: &Path) -> Vec<Note>;

    /// Файлы шрифтов в vault (фича 005, FR-027, FR-029).
    ///
    /// Отдельный обход, а не выборка из индекса: индекс хранит то, что
    /// показывается в дереве, а шрифты в дереве не показываются (spec.md →
    /// Assumptions). Спрашивается на каждую сборку, поэтому обход читает
    /// только метаданные — сами байты сборка берёт через `read_bytes` и лишь
    /// тогда, когда отпечаток файла изменился.
    ///
    /// Скрытые папки пропускаются тем же правилом, что и в обходе заметок.
    async fn list_fonts(&self) -> Vec<super::FontFile>;

    /// Все файлы под корнем с размерами, кроме скрытых записей (фича 021,
    /// FR-002, research.md §9).
    ///
    /// Нужен импорту: источник — обычная папка Markdown, а `list()` отдаёт
    /// только заметки Typst и доски. Для vault назначения тот же обход даёт
    /// занятые имена. Читает только метаданные; символические ссылки не
    /// раскрываются.
    ///
    /// Умолчание — пусто: двойникам, которым импорт не нужен, обход незачем.
    async fn list_source_files(&self) -> crate::vault::SourceListing {
        crate::vault::SourceListing::default()
    }

    /// Конфликтные копии Syncthing в vault (фича 011, FR-024), пути
    /// относительно корня.
    ///
    /// Отдельный обход, а не выборка из индекса: в индекс копии не попадают
    /// намеренно (`classify` относит их к `Other`), а найти их нужно и внутри
    /// служебного места — копия `vault.json` означает спор о хранителе
    /// (FR-032). Каталог истории не обходится.
    ///
    /// Умолчание — копий нет: тестовым двойникам, которым синхронизация не
    /// нужна, реализовывать обход незачем.
    async fn list_conflict_copies(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Изображения в vault — для сопоставления вложений по содержимому (фича
    /// 016, research.md §7). `None` — весь vault, по правилам обхода шрифтов
    /// (скрытые папки и символические ссылки пропускаются); `Some(folder)` —
    /// файлы одной папки без вложенных, включая скрытую. Читаются только
    /// метаданные; порядок — по пути.
    ///
    /// Умолчание — пусто: двойникам, которым вложения не нужны, обход незачем.
    async fn list_images(&self, _folder: Option<&Path>) -> Vec<crate::vault::ImageFile> {
        Vec::new()
    }

    /// Кандидаты в файлы источников (фича 017, research.md §3): `.bib`,
    /// `.yml`, `.yaml` вида `Other`, вне скрытых папок. Только метаданные —
    /// читает файлы библиотека, и только изменившиеся.
    ///
    /// Умолчание — пусто: двойникам без источников обход незачем.
    async fn list_source_candidates(&self) -> Vec<crate::sources::SourceCandidate> {
        Vec::new()
    }

    /// Проходит ли путь через символическую ссылку-каталог (фича 011, T068).
    ///
    /// Запись по относительному пути обязана оставаться внутри vault, но
    /// ссылка-каталог на полпути уводит её куда угодно. Синхронизация пишет
    /// файлы по путям из чужого репозитория и перед каждой записью и
    /// удалением спрашивает об этом. Сама последняя часть пути ссылкой быть
    /// может: атомарная запись заменяет ссылку файлом, а не пишет сквозь неё.
    ///
    /// Умолчание — нет: у хранилищ без символических ссылок (File System
    /// Access API, двойники в тестах) пройти сквозь неё нечем.
    async fn crosses_symlink(&self, _path: &Path) -> bool {
        false
    }

    /// Является ли сам файл по этому пути символической ссылкой (фича 012,
    /// FR-016).
    ///
    /// В отличие от [`Self::crosses_symlink`], вопрос — про последнюю часть
    /// пути: редактор правил показывает содержимое, прочитанное сквозь
    /// ссылку, а атомарная запись заменила бы ссылку файлом — править такое
    /// молча нельзя.
    ///
    /// Умолчание — нет, по той же причине, что и там.
    async fn is_symlink(&self, _path: &Path) -> bool {
        false
    }

    /// Размер и время изменения файла (мс) без чтения содержимого; `None` —
    /// файла нет или хранилище таких сведений не даёт (фича 005, FR-020).
    ///
    /// Нужно предпросмотру, чтобы заметить правку связанного файла, которого
    /// нет в индексе заметок — изображения, файла данных: читать их целиком
    /// раз в секунду ради вопроса «не изменился ли» слишком дорого.
    ///
    /// Умолчание — `None`: без этих сведений предпросмотр просто не узнаёт о
    /// правке сам и ждёт следующей сборки.
    async fn file_mark(&self, _path: &Path) -> Option<(u64, i64)> {
        None
    }

    /// Состояние папки внутри vault: есть ли она, читается ли, пишется ли
    /// (фича 003, FR-004, FR-009).
    ///
    /// Отдельный вопрос, а не вывод из `list()`. Обход возвращает файлы, а не
    /// папки, и «папки нет» неотличимо в нём от «папка есть, но пуста» и от
    /// «папка есть, но не читается». Галерее заготовок эти три случая нужно
    /// различать: первый чинится первым же сохранением, второй — штатное
    /// состояние, третий — поломка, о которой надо сказать.
    ///
    /// Побочных эффектов не имеет: файлов-пробников не создаёт (та же причина,
    /// что у `probe_access_state` в `native.rs`).
    async fn folder_access(&self, folder: &Path) -> crate::vault::FolderAccess;
    async fn read(&self, path: &Path) -> Result<String, ReadError>;

    /// Содержимое файла байтами — для того, что текстом не является.
    ///
    /// Нужно предпросмотру (фича 005): `#image("схема.png")` собирается из
    /// байтов, и пропускать их через `String` нельзя — не всякий файл
    /// UTF-8. Отдельный метод, а не замена `read`: заметки, конфликты и хэши
    /// работают с текстом, и переводить их на байты значило бы гонять
    /// перекодировку на каждом чтении.
    ///
    /// Пределы и классификация отказов те же, что у `read` (принцип II).
    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, ReadError>;

    /// Содержимое файла без предела размера заметки (фича 011, T073).
    ///
    /// Предел [`crate::vault::NOTE_SIZE_LIMIT_BYTES`] защищает редактор и
    /// предпросмотр: заметку на десятки мегабайт им открывать незачем. Истории
    /// он не указ — вложение или архив в vault обязаны сравниваться и
    /// сливаться при получении как любой другой файл, а не обрывать его.
    ///
    /// Умолчание — обычное чтение: хранилищу без предела различать нечего.
    async fn read_bytes_any_size(&self, path: &Path) -> Result<Vec<u8>, ReadError> {
        self.read_bytes(path).await
    }

    /// `expected` — снимок диска, от которого отталкивался редактор. Если снимок
    /// на диске разошёлся — `WriteError::Conflict`, а не молчаливая перезапись
    /// (FR-021).
    async fn write(
        &mut self,
        path: &Path,
        content: &str,
        expected: Option<FileStamp>,
    ) -> Result<(), WriteError>;

    /// Записывает файл **байтами** — для того, что текстом не является.
    ///
    /// Парная к [`VaultBackend::read_bytes`] и появилась по той же причине:
    /// не всякий файл vault — UTF-8. Понадобилась загрузке внешних шаблонов
    /// (фича 007): шаблон Typst обычно содержит не только `.typ`, но и
    /// картинки и шрифты, и пропускать их через `String` нельзя — они
    /// перестанут быть собой.
    ///
    /// Снимка `expected` здесь нет намеренно: обнаружение внешних изменений
    /// (001 FR-021) касается заметок, которые пользователь редактирует, а не
    /// файлов, которые приложение раскладывает целиком. Конфликтовать с самим
    /// собой этой записи не с чем.
    ///
    /// **Запись атомарна** на обоих таргетах (фича 009, research.md §4): на
    /// месте файла в любой момент лежит либо прежнее содержимое целиком, либо
    /// новое целиком. Native пишет во временный файл и переименовывает его на
    /// место; web получает то же от `FileSystemWritableFileStream`, который
    /// пишет в swap-файл и подменяет оригинал только на `close()`. Экспорт PDF
    /// на этом стоит: отмена пакета не имеет права оставить обрезанный файл.
    async fn write_bytes(&mut self, path: &Path, bytes: &[u8]) -> Result<(), WriteError>;

    async fn create(&mut self, path: &Path) -> Result<(), CreateError>;

    /// Создаёт папку внутри vault, если её ещё нет (фича 002, FR-019).
    ///
    /// Появилась вместе со снимками вставок: снимок лежит в
    /// `<заметка>.assets/`, и эту папку создаёт первая же вставка
    /// (research.md §4 фичи 002). Отдельная операция, а не создание папок внутри
    /// `write`: «записать в несуществующую папку» обязано оставаться отказом
    /// (001 FR-014), иначе опечатка в пути молча заводит папку в vault
    /// пользователя. Существующая папка — не ошибка: вторая вставка в ту же
    /// заметку вызывает это на уже созданной папке.
    async fn ensure_folder(&mut self, path: &Path) -> Result<(), WriteError>;
    async fn rename(&mut self, from: &Path, to: &Path) -> Result<(), RenameError>;
    async fn delete(&mut self, path: &Path) -> Result<(), DeleteError>;

    /// Папки, которые могут принять `from`, в порядке показа (фича 023, FR-012,
    /// FR-022); `query` — фильтр, пустой — все. Правила — в
    /// [`crate::vault::relocate::move_targets`].
    async fn move_targets(&self, from: &Path, query: &str) -> Vec<MoveTarget>;

    /// Что сломается, если выполнить перенос (FR-016, FR-017). Отказы те же, что
    /// у [`VaultBackend::move_entry`]: показывать подтверждение переноса,
    /// который всё равно будет отклонён, незачем.
    async fn move_impact(&self, request: &MoveRequest) -> Result<MoveImpact, MoveRefusal>;

    /// Переносит заметку, доску или папку в другую папку vault, не меняя имени
    /// (фича 023). Отдельная операция, а не `rename`: contracts/core-move-api.md.
    ///
    /// Гарантии: при `Err` и `Cancelled` файлы vault прежние; при `Done`
    /// содержимое перенесённого побайтно прежнее, а индекс, правки, таблица
    /// ссылок и пометки разделов уже следуют за новым путём; как внешнее
    /// изменение или удаление перенос не показывается (FR-008).
    async fn move_entry(
        &mut self,
        request: &MoveRequest,
        control: &dyn MoveControl,
    ) -> Result<MoveOutcome, MoveError>;

    /// Делегирует модулю `graph` (research.md §8) — vault-manager не парсит
    /// содержимое заметок сам.
    async fn backlinks(&self, path: &Path) -> Vec<Note>;

    /// Цели ссылок заметки, которых нет среди заметок vault (FR-030). Именно
    /// это позволяет пользователю найти битые ссылки после операции: сама
    /// таблица ссылок «битости» не знает — её определяет индекс.
    async fn broken_links(&self, path: &Path) -> Vec<PathBuf>;

    /// Забирает накопившиеся с прошлого вызова события внешних изменений
    /// (FR-016) — реализация `watch()` из contracts/vault-backend-trait.md в виде
    /// pull-опроса вместо push-потока, чтобы не требовать object-safe Stream.
    fn poll_events(&mut self) -> Vec<VaultEvent>;

    /// Редактор объявляет ядру несохранённые правки и передаёт сам буфер
    /// (FR-035: только в памяти работающего приложения). Без этого решение
    /// «поднять конфликт или тихо обновить» пришлось бы принимать в UI, где
    /// ему не место (принцип I): именно наличие буфера отличает `FR-017` от
    /// `FR-021`.
    async fn begin_edit(&mut self, path: &Path, buffer: String);

    /// Правки отброшены самим редактором (закрытие без сохранения по явному
    /// действию пользователя) — снимает буфер и всё, что от него зависело.
    async fn discard_edit(&mut self, path: &Path);

    fn has_unsaved_edits(&self, path: &Path) -> bool;

    /// Неразрешённый конфликт по заметке (FR-021), если он есть.
    fn pending_conflict(&self, path: &Path) -> Option<EditConflict>;

    /// Всё, что будет потеряно при закрытии окна прямо сейчас (FR-035, FR-037):
    /// неразрешённые конфликты, внешние удаления с правками и просто
    /// несохранённые буферы.
    fn unresolved_edit_paths(&self) -> Vec<PathBuf>;

    // ── Свойства заметок (фича 020) ───────────────────────────────────────

    /// Сведения о заметках для отбора по свойствам: путь, встроенные сведения
    /// и свойства из индекса (research.md §4).
    ///
    /// Умолчание читает все заметки и разбирает их заново — верно для любого
    /// backend, но медленно; `NativeBackend` и `WebBackend` отвечают из индекса.
    async fn indexed_notes(&self) -> Vec<crate::properties::IndexedNote> {
        crate::properties::index::scan_notes(self).await
    }

    /// Записывает свойства заметки в индекс (пакетный пересчёт, T016).
    /// Умолчание ничего не хранит: у такого backend индекса свойств нет.
    async fn set_indexed_properties(
        &mut self,
        _path: &Path,
        _entries: Vec<crate::properties::PropertyEntry>,
    ) {
    }

    /// Сведения о свойствах в индексе построены по текущему формату
    /// (`PROPERTIES_FORMAT_VERSION`). `false` — нужен пакетный пересчёт.
    async fn properties_current(&self) -> bool {
        true
    }

    /// Отмечает, что пересчёт свойств дошёл до конца.
    async fn mark_properties_current(&mut self) {}

    /// Всё, из чего строится граф связей (фича 025): заметки, доски, разрешённые
    /// ссылки и вставки файлов. Умолчание читает все заметки и разбирает их
    /// заново; `NativeBackend` и `WebBackend` отвечают из индекса, не читая файлы.
    async fn graph_source(&self) -> crate::graph::view::GraphSource {
        crate::graph::view::scan_graph_source(self).await
    }

    /// Отличия между версией пользователя и версией на диске (FR-024). Сам по
    /// себе конфликт не разрешает — пользователь возвращается к выбору.
    async fn conflict_diff(&self, path: &Path) -> Vec<DiffLine>;

    /// «Оставить свою версию» (FR-022, US4 AS2): записывает локальный буфер на
    /// диск без `expected`-снимка — пользователь уже сделал явный выбор
    /// перезаписать внешнюю версию.
    async fn resolve_conflict_keep_local(&mut self, path: &Path) -> Result<(), ResolveError>;

    /// «Принять внешнюю» (FR-022, FR-023, US4 AS3): отбрасывает локальный буфер
    /// и возвращает содержимое, которое редактор обязан показать вместо него.
    /// `Ok(None)` — внешней версии не существует: заметка удалена снаружи
    /// (FR-025), и «принять внешнюю» означает принять удаление.
    async fn resolve_conflict_accept_external(
        &mut self,
        path: &Path,
    ) -> Result<Option<String>, ResolveError>;
}

/// Обёртка «только чтение» над любым backend (фича 021, FR-002, SC-005,
/// research.md §9).
///
/// Источник импорта — чужая папка, которую приложение обязано оставить
/// побайтно прежней. Гарантия сделана типом, а не дисциплиной вызывающего:
/// каждый метод записи здесь отвечает отказом `AccessDenied` и до внутреннего
/// backend не доходит, а [`VaultBackend::access_state`] всегда `ReadOnly`.
///
/// Обёртка не открывает папку сама: `open` на ней отказывает. Backend для
/// чтения создаётся своим конструктором (`NativeBackend::open_read_only`,
/// `WebBackend` из хендла), который не заводит `.typsidian/`, кэш и слежение —
/// иначе сама попытка открыть источник изменила бы его.
pub struct ReadOnly<B: VaultBackend> {
    inner: B,
}

impl<B: VaultBackend> ReadOnly<B> {
    pub fn new(inner: B) -> Self {
        Self { inner }
    }

    /// Внутренний backend — только для чтения: изменяющих методов у `&B` нет.
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

impl<B: VaultBackend> VaultBackend for ReadOnly<B> {
    async fn open(_root_path: &Path) -> Result<Self, OpenError> {
        Err(OpenError::AccessDenied)
    }

    fn root_path(&self) -> &Path {
        self.inner.root_path()
    }

    fn access_state(&self) -> AccessState {
        AccessState::ReadOnly
    }

    fn service_dir_existed_at_open(&self) -> bool {
        self.inner.service_dir_existed_at_open()
    }

    async fn list(&self) -> Vec<Note> {
        self.inner.list().await
    }

    async fn list_folder(&self, folder: &Path) -> Vec<Note> {
        self.inner.list_folder(folder).await
    }

    async fn list_fonts(&self) -> Vec<super::FontFile> {
        self.inner.list_fonts().await
    }

    async fn list_source_files(&self) -> crate::vault::SourceListing {
        self.inner.list_source_files().await
    }

    async fn list_conflict_copies(&self) -> Vec<PathBuf> {
        self.inner.list_conflict_copies().await
    }

    async fn list_images(&self, folder: Option<&Path>) -> Vec<crate::vault::ImageFile> {
        self.inner.list_images(folder).await
    }

    async fn list_source_candidates(&self) -> Vec<crate::sources::SourceCandidate> {
        self.inner.list_source_candidates().await
    }

    async fn crosses_symlink(&self, path: &Path) -> bool {
        self.inner.crosses_symlink(path).await
    }

    async fn is_symlink(&self, path: &Path) -> bool {
        self.inner.is_symlink(path).await
    }

    async fn file_mark(&self, path: &Path) -> Option<(u64, i64)> {
        self.inner.file_mark(path).await
    }

    async fn folder_access(&self, folder: &Path) -> crate::vault::FolderAccess {
        match self.inner.folder_access(folder).await {
            // Писать сюда нельзя, даже если внутренний backend мог бы.
            crate::vault::FolderAccess::Present => crate::vault::FolderAccess::NotWritable {
                reason: crate::vault::FolderProblem::VaultReadOnly,
            },
            other => other,
        }
    }

    async fn read(&self, path: &Path) -> Result<String, ReadError> {
        self.inner.read(path).await
    }

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, ReadError> {
        self.inner.read_bytes(path).await
    }

    async fn read_bytes_any_size(&self, path: &Path) -> Result<Vec<u8>, ReadError> {
        self.inner.read_bytes_any_size(path).await
    }

    async fn write(
        &mut self,
        _path: &Path,
        _content: &str,
        _expected: Option<FileStamp>,
    ) -> Result<(), WriteError> {
        Err(WriteError::AccessDenied)
    }

    async fn write_bytes(&mut self, _path: &Path, _bytes: &[u8]) -> Result<(), WriteError> {
        Err(WriteError::AccessDenied)
    }

    async fn create(&mut self, _path: &Path) -> Result<(), CreateError> {
        Err(CreateError::AccessDenied)
    }

    async fn ensure_folder(&mut self, _path: &Path) -> Result<(), WriteError> {
        Err(WriteError::AccessDenied)
    }

    async fn rename(&mut self, _from: &Path, _to: &Path) -> Result<(), RenameError> {
        Err(RenameError::AccessDenied)
    }

    async fn delete(&mut self, _path: &Path) -> Result<(), DeleteError> {
        Err(DeleteError::AccessDenied)
    }

    async fn move_targets(&self, from: &Path, query: &str) -> Vec<MoveTarget> {
        self.inner.move_targets(from, query).await
    }

    async fn move_impact(&self, request: &MoveRequest) -> Result<MoveImpact, MoveRefusal> {
        self.inner.move_impact(request).await
    }

    async fn move_entry(
        &mut self,
        _request: &MoveRequest,
        _control: &dyn MoveControl,
    ) -> Result<MoveOutcome, MoveError> {
        Err(MoveRefusal::AccessDenied.into())
    }

    async fn backlinks(&self, path: &Path) -> Vec<Note> {
        self.inner.backlinks(path).await
    }

    async fn broken_links(&self, path: &Path) -> Vec<PathBuf> {
        self.inner.broken_links(path).await
    }

    fn poll_events(&mut self) -> Vec<VaultEvent> {
        self.inner.poll_events()
    }

    async fn begin_edit(&mut self, _path: &Path, _buffer: String) {}

    async fn discard_edit(&mut self, _path: &Path) {}

    fn has_unsaved_edits(&self, _path: &Path) -> bool {
        false
    }

    fn pending_conflict(&self, _path: &Path) -> Option<EditConflict> {
        None
    }

    fn unresolved_edit_paths(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    async fn indexed_notes(&self) -> Vec<crate::properties::IndexedNote> {
        self.inner.indexed_notes().await
    }

    async fn graph_source(&self) -> crate::graph::view::GraphSource {
        self.inner.graph_source().await
    }

    async fn conflict_diff(&self, _path: &Path) -> Vec<DiffLine> {
        Vec::new()
    }

    async fn resolve_conflict_keep_local(&mut self, _path: &Path) -> Result<(), ResolveError> {
        Err(ResolveError::AccessDenied)
    }

    async fn resolve_conflict_accept_external(
        &mut self,
        _path: &Path,
    ) -> Result<Option<String>, ResolveError> {
        Err(ResolveError::AccessDenied)
    }
}
