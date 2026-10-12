//! Вкладки и разделение редактора (фича 014).
//!
//! Модуль устроен по образцу пакетного экспорта (`export::batch`): состояние —
//! значение, которое держит окно и передаёт ядру туда и обратно; единственная
//! точка изменения — [`apply`], единственная точка показа — [`view`]. Обе
//! чистые: сведения о заметках приходят готовыми в [`Facts`], собирает их
//! [`facts::facts_of`] из индекса бэкенда. Платформенного кода здесь нет
//! (принцип II), биндинги wasm — в `web.rs`.
//!
//! Что в модели **не** живёт: текст заметок, каретка и прокрутка. Это
//! принадлежит редактору (research.md §4); набор текста модель не вызывает
//! вовсе — действия «текст изменился» не существует (FR-027).

pub mod facts;
pub mod history;
pub mod model;
pub mod persist;
pub mod view;

#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};

use crate::vault::FileKind;

pub use facts::{apply_with, facts_of, restore_with, view_with};
pub use model::{apply, check_invariants};
pub use persist::{restore, snapshot, SavedGroup, SavedWorkspace};
pub use view::view;

/// Наименьшая ширина окна, при которой две группы показываются рядом
/// (макет 1b, FR-020).
pub const SPLIT_MIN_WIDTH_PX: u32 = 900;
/// Наибольшая ширина «телефона»: полосы вкладок нет, открытые заметки — шторкой
/// (FR-024). Совпадает с точкой перелома `app.css` (`max-width: 720px`), то есть
/// включительно.
pub const COMPACT_MAX_WIDTH_PX: u32 = 720;
/// Длина истории переходов одной группы (FR-010).
pub const HISTORY_LIMIT: usize = 100;
/// Границы доли первой группы, промилле (FR-018).
pub const RATIO_MIN_PERMILLE: u16 = 200;
pub const RATIO_MAX_PERMILLE: u16 = 800;
/// Доля первой группы по умолчанию.
pub const RATIO_DEFAULT_PERMILLE: u16 = 500;

/// Идентификатор вкладки. Не меняется при переименовании заметки и при
/// переносе между группами (FR-016), поэтому по нему интерфейс держит снимок
/// состояния редактора.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TabId(pub u32);

/// Место группы: первая или вторая.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum GroupSlot {
    First,
    Second,
}

impl GroupSlot {
    pub(crate) fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
        }
    }

    pub(crate) fn of_index(index: usize) -> Self {
        if index == 0 {
            Self::First
        } else {
            Self::Second
        }
    }

    /// Соседняя группа.
    pub fn other(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::First,
        }
    }
}

/// Место в тексте: строка и колонка с единицы — те же единицы, что у
/// `preview::DiagnosticLocation` и перехода `reveal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TextPlace {
    pub line: u32,
    pub column: u32,
}

/// Вкладка: заметка или доска.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Tab {
    pub id: TabId,
    /// Путь в vault.
    pub path: String,
    pub kind: FileKind,
}

/// Запись истории переходов: заметка и место, на котором стояли.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct HistoryEntry {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub place: Option<TextPlace>,
}

/// История переходов одной группы (FR-010…FR-014): записи и указатель на
/// текущую. Всё правее указателя — «вперёд».
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct History {
    pub entries: Vec<HistoryEntry>,
    pub cursor: usize,
}

/// Группа вкладок.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Group {
    /// Порядок полосы; путь в группе уникален (FR-002).
    pub tabs: Vec<Tab>,
    /// `None` только у пустой единственной группы.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub active: Option<TabId>,
    /// Порядок активности, свежая первой (FR-006).
    pub recent: Vec<TabId>,
    pub history: History,
}

/// Состояние рабочей области окна (data-model.md «WorkspaceState»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct WorkspaceState {
    /// Одна или две группы; вторая не бывает пустой.
    pub groups: Vec<Group>,
    pub active_group: GroupSlot,
    /// Доля первой группы, промилле: 200…800.
    pub ratio_permille: u16,
    /// Счётчик выдачи [`TabId`].
    pub next_tab: u32,
}

impl WorkspaceState {
    /// Пустое состояние: одна пустая группа.
    pub fn empty() -> Self {
        Self {
            groups: vec![Group::default()],
            active_group: GroupSlot::First,
            ratio_permille: RATIO_DEFAULT_PERMILLE,
            next_tab: 1,
        }
    }
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self::empty()
    }
}

/// Сторона обхода вкладок.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Direction {
    Next,
    Previous,
}

/// Действие над рабочей областью (data-model.md «WorkspaceAction»).
///
/// `leaving` — место каретки заметки, с которой уходят: его знает редактор, а
/// не модель, и оно нужно истории, чтобы «Назад» вернул на место (FR-011).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum WorkspaceAction {
    /// Открыть путь вкладкой активной группы либо активировать существующую.
    Open {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        target: Option<TextPlace>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        leaving: Option<TextPlace>,
    },
    Activate {
        tab: TabId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        leaving: Option<TextPlace>,
    },
    Cycle {
        direction: Direction,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        leaving: Option<TextPlace>,
    },
    /// Закрыть с проверкой (FR-005). `board_unsaved` — состояние сохранения
    /// доски после дописывания: индекс ядра её несохранённого не видит
    /// (research.md §6).
    Close {
        tab: TabId,
        board_unsaved: bool,
    },
    CloseOthers {
        tab: TabId,
    },
    /// Порядок в полосе и перенос между группами.
    Move {
        tab: TabId,
        group: GroupSlot,
        index: u32,
    },
    MoveToOtherGroup {
        tab: TabId,
    },
    Split {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        leaving: Option<TextPlace>,
    },
    Unsplit,
    FocusGroup {
        group: GroupSlot,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        leaving: Option<TextPlace>,
    },
    SetRatio {
        permille: u16,
    },
    Back {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        leaving: Option<TextPlace>,
    },
    Forward {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        leaving: Option<TextPlace>,
    },
    /// Событие vault: заметку переименовали.
    Renamed {
        from: String,
        to: String,
    },
    /// Событие vault: заметку удалили.
    Deleted {
        path: String,
    },
}

/// Сведения о заметке, нужные переходу.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct NoteFact {
    pub path: String,
    pub kind: FileKind,
    /// Есть несохранённые правки.
    pub unsaved: bool,
    /// Есть неразрешённый конфликт (или удаление с правками).
    pub conflict: bool,
}

/// Факты, по которым решают переходы и показ. Только пути, названные
/// состоянием и действием: отсутствие записи означает «заметки нет».
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Facts {
    pub notes: Vec<NoteFact>,
    /// Ширина окна, px.
    pub width_px: u32,
}

impl Facts {
    pub fn note(&self, path: &str) -> Option<&NoteFact> {
        self.notes.iter().find(|fact| fact.path == path)
    }
}

/// Куда поставить каретку после перехода.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Focus {
    pub group: GroupSlot,
    pub tab: TabId,
    pub path: String,
    pub kind: FileKind,
    /// Задано при `Open` с `target` и при `Back`/`Forward`, если у записи
    /// истории есть место.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub place: Option<TextPlace>,
}

/// Почему вкладка не закрылась (FR-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum RefusalReason {
    Unsaved,
    Conflict,
}

/// Вкладка, которая не закрылась.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CloseRefusal {
    pub tab: TabId,
    pub path: String,
    pub reason: RefusalReason,
}

/// Почему действие отклонено целиком; состояние при этом прежнее.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Rejection {
    /// Заметки нет (или это не заметка и не доска).
    NotFound,
    /// Окно уже порога [`SPLIT_MIN_WIDTH_PX`].
    TooNarrow,
    /// Делить нечего: нет активной вкладки, единственная вкладка — доска или
    /// редактор уже разделён.
    NothingToSplit,
    /// Действие требует двух групп (или такой группы нет).
    SingleGroup,
    /// Двигаться по истории некуда.
    NoHistory,
    UnknownTab,
}

/// Итог перехода.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Transition {
    /// Новое состояние; при отказе — прежнее.
    pub state: WorkspaceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub focus: Option<Focus>,
    /// Вкладки, которые не закрылись.
    pub refused: Vec<CloseRefusal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub rejected: Option<Rejection>,
    /// Закрытые вкладки: интерфейс освобождает их снимки редактора.
    pub closed: Vec<TabId>,
}

/// Подача вкладок (research.md §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Presentation {
    /// Две полосы рядом.
    Split,
    /// Одна полоса (группа одна либо окно слишком узкое для двух).
    Single,
    /// Телефон: полосы нет, открытые заметки — списком в шторке.
    List,
}

/// Вкладка в показе.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TabView {
    pub id: TabId,
    /// Настоящая группа вкладки: в слитой полосе они разные.
    pub group: GroupSlot,
    pub path: String,
    /// Имя без расширения.
    pub title: String,
    /// Кратчайший отличающий хвост папки, если `title` повторяется среди
    /// показанных вкладок (FR-007).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub hint: Option<String>,
    pub kind: FileKind,
    pub unsaved: bool,
    pub conflict: bool,
    /// Активна в своей группе.
    pub active: bool,
}

/// Полоса вкладок в показе.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct GroupView {
    pub slot: GroupSlot,
    pub tabs: Vec<TabView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub active: Option<TabId>,
    pub can_back: bool,
    pub can_forward: bool,
}

/// Заметка с кареткой: за ней следуют предпросмотр и панели (FR-009).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ActiveNote {
    pub tab: TabId,
    pub path: String,
    /// Имя файла.
    pub name: String,
    pub kind: FileKind,
}

/// Что показывать (data-model.md «WorkspaceView»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct WorkspaceView {
    pub presentation: Presentation,
    pub groups: Vec<GroupView>,
    pub active_group: GroupSlot,
    pub ratio_permille: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub active: Option<ActiveNote>,
    pub can_split: bool,
    pub can_unsplit: bool,
    pub can_move_to_other: bool,
    /// Вкладок во всех показанных полосах.
    pub tab_count: u32,
}
