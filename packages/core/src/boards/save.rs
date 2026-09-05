//! Состояние сохранения открытой доски (фича 002, FR-009…FR-011,
//! data-model.md «BoardSaveState»).
//!
//! Машина, а не флаг: пользователь должен различать «есть несохранённые
//! изменения», «идёт запись» и «записать не удалось» (FR-009), а последнее
//! состояние обязано пережить попытку закрыть окно (FR-011).
//!
//! Ключевой инвариант: **из `Failed` нет автоматического перехода в `Clean`**.
//! Неудавшаяся запись не «рассасывается» сама; состояние снимается только новой
//! правкой и последующей успешной записью. Иначе индикатор погас бы сам, и
//! пользователь закрыл бы окно, считая работу сохранённой.

use super::events::BoardSaveState;

/// Отслеживает состояние сохранения одной открытой доски.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveTracker {
    state: BoardSaveState,
}

impl Default for SaveTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SaveTracker {
    pub fn new() -> Self {
        Self {
            state: BoardSaveState::Clean,
        }
    }

    pub fn state(&self) -> &BoardSaveState {
        &self.state
    }

    /// Пользователь изменил рисунок.
    ///
    /// Работает из любого состояния, включая `Failed`: правка после неудачной
    /// записи — обычное продолжение работы, и следующая запись должна её
    /// подхватить.
    pub fn edited(&mut self) {
        self.state = BoardSaveState::Dirty;
    }

    /// Начать запись. Возвращает `false`, если писать нечего.
    ///
    /// Из `Clean` запись не начинается: дебаунс мог сработать после того, как
    /// предыдущая запись уже прошла, и лишний проход по диску здесь не нужен.
    /// Из `Saving` тоже: параллельные записи одного файла — гонка.
    pub fn start_saving(&mut self) -> bool {
        match self.state {
            BoardSaveState::Dirty | BoardSaveState::Failed { .. } => {
                self.state = BoardSaveState::Saving;
                true
            }
            BoardSaveState::Clean | BoardSaveState::Saving => false,
        }
    }

    /// Запись прошла.
    ///
    /// Переход разрешён только из `Saving`: объявить доску сохранённой, не
    /// начав записи, невозможно.
    pub fn saved(&mut self) {
        if self.state == BoardSaveState::Saving {
            self.state = BoardSaveState::Clean;
        }
    }

    /// Запись не удалась. Содержимое полотна при этом не теряется (FR-010) —
    /// буфер принадлежит движку и ядром не трогается.
    pub fn failed(&mut self, reason: impl Into<String>) {
        self.state = BoardSaveState::Failed {
            reason: reason.into(),
        };
    }

    /// Нужно ли предупредить перед закрытием окна (FR-011).
    ///
    /// `Saving` тоже удерживает: запись могла не дойти до диска.
    pub fn blocks_closing(&self) -> bool {
        !matches!(self.state, BoardSaveState::Clean)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_returns_to_clean() {
        let mut tracker = SaveTracker::new();
        assert_eq!(tracker.state(), &BoardSaveState::Clean);

        tracker.edited();
        assert_eq!(tracker.state(), &BoardSaveState::Dirty);

        assert!(tracker.start_saving());
        assert_eq!(tracker.state(), &BoardSaveState::Saving);

        tracker.saved();
        assert_eq!(tracker.state(), &BoardSaveState::Clean);
        assert!(!tracker.blocks_closing());
    }

    #[test]
    fn failure_is_terminal_until_the_next_edit() {
        // FR-010, FR-011: неудавшаяся запись не рассасывается сама.
        let mut tracker = SaveTracker::new();
        tracker.edited();
        tracker.start_saving();
        tracker.failed("нет прав на запись");

        assert!(matches!(tracker.state(), BoardSaveState::Failed { .. }));
        assert!(tracker.blocks_closing());

        // `saved()` без начатой записи ничего не меняет — иначе состояние
        // можно было бы снять, ничего не записав.
        tracker.saved();
        assert!(matches!(tracker.state(), BoardSaveState::Failed { .. }));

        tracker.edited();
        assert_eq!(tracker.state(), &BoardSaveState::Dirty);
    }

    #[test]
    fn a_failed_board_can_be_saved_again() {
        let mut tracker = SaveTracker::new();
        tracker.edited();
        tracker.start_saving();
        tracker.failed("диск был занят");

        // Повтор возможен и без новой правки: причина отказа могла исчезнуть.
        assert!(tracker.start_saving());
        tracker.saved();
        assert_eq!(tracker.state(), &BoardSaveState::Clean);
    }

    #[test]
    fn saving_does_not_start_from_clean_or_restart_while_in_flight() {
        let mut tracker = SaveTracker::new();
        assert!(!tracker.start_saving(), "из Clean писать нечего");

        tracker.edited();
        assert!(tracker.start_saving());
        assert!(
            !tracker.start_saving(),
            "параллельная запись того же файла — гонка"
        );
    }

    #[test]
    fn every_state_but_clean_holds_the_window() {
        // FR-011: предупреждение перед закрытием.
        let mut tracker = SaveTracker::new();
        assert!(!tracker.blocks_closing());

        tracker.edited();
        assert!(tracker.blocks_closing());

        tracker.start_saving();
        assert!(tracker.blocks_closing());

        tracker.failed("отказ");
        assert!(tracker.blocks_closing());
    }
}
