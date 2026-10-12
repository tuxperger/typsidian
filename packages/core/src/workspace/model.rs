//! Переходы рабочей области: [`apply`] (data-model.md «Переходы», contracts/
//! core-workspace-api.md).
//!
//! Чистая функция: состояние и действие на входе, [`Transition`] на выходе;
//! сведения о заметках приходят в [`Facts`]. Не паникует ни на каком сочетании
//! состояния и действия — неизвестная вкладка, отсутствующая группа, действие
//! в пустом состоянии дают `rejected` и прежнее состояние.
//!
//! Устройство: [`Ctx`] держит рабочую копию состояния и накопленный итог;
//! каждое действие — метод, возвращающий `Result<(), Rejection>`. Отказ
//! отбрасывает рабочую копию целиком, поэтому «полуприменённых» переходов нет.

use super::{
    CloseRefusal, Direction, Facts, Focus, Group, GroupSlot, HistoryEntry, RefusalReason,
    Rejection, Tab, TabId, TextPlace, Transition, WorkspaceAction, WorkspaceState,
    RATIO_MAX_PERMILLE, RATIO_MIN_PERMILLE, SPLIT_MIN_WIDTH_PX,
};
use crate::vault::FileKind;

/// Применяет действие. См. описание модуля.
pub fn apply(state: WorkspaceState, action: &WorkspaceAction, facts: &Facts) -> Transition {
    let mut cx = Ctx {
        state: state.clone(),
        facts,
        focus: None,
        refused: Vec::new(),
        closed: Vec::new(),
    };
    match cx.run(action) {
        Ok(()) => Transition {
            state: cx.state,
            focus: cx.focus,
            refused: cx.refused,
            rejected: None,
            closed: cx.closed,
        },
        Err(rejection) => Transition {
            state,
            focus: None,
            refused: Vec::new(),
            rejected: Some(rejection),
            closed: Vec::new(),
        },
    }
}

/// Открываются вкладкой только заметки и доски.
fn is_openable(kind: FileKind) -> bool {
    matches!(kind, FileKind::Note | FileKind::Board)
}

/// Две группы, но окно уже порога: показана одна слитая полоса (research.md
/// §10).
pub(crate) fn is_merged(state: &WorkspaceState, facts: &Facts) -> bool {
    state.groups.len() == 2 && facts.width_px < SPLIT_MIN_WIDTH_PX
}

struct Ctx<'a> {
    state: WorkspaceState,
    facts: &'a Facts,
    focus: Option<Focus>,
    refused: Vec<CloseRefusal>,
    closed: Vec<TabId>,
}

impl Ctx<'_> {
    fn run(&mut self, action: &WorkspaceAction) -> Result<(), Rejection> {
        match action {
            WorkspaceAction::Open {
                path,
                target,
                leaving,
            } => self.open(path, *target, *leaving),
            WorkspaceAction::Activate { tab, leaving } => {
                let (group, _) = self.find(*tab).ok_or(Rejection::UnknownTab)?;
                self.navigate(group, *tab, *leaving, None);
                Ok(())
            }
            WorkspaceAction::Cycle { direction, leaving } => {
                self.cycle(*direction, *leaving);
                Ok(())
            }
            WorkspaceAction::Close { tab, board_unsaved } => {
                let (group, _) = self.find(*tab).ok_or(Rejection::UnknownTab)?;
                self.close_checked(group, *tab, *board_unsaved, true);
                Ok(())
            }
            WorkspaceAction::CloseOthers { tab } => self.close_others(*tab),
            WorkspaceAction::Move { tab, group, index } => {
                self.move_tab(*tab, *group, *index as usize)
            }
            WorkspaceAction::MoveToOtherGroup { tab } => {
                let (group, _) = self.find(*tab).ok_or(Rejection::UnknownTab)?;
                if self.state.groups.len() < 2 {
                    return Err(Rejection::SingleGroup);
                }
                self.move_tab(*tab, GroupSlot::of_index(group).other(), usize::MAX)
            }
            WorkspaceAction::Split { leaving } => self.split(*leaving),
            WorkspaceAction::Unsplit => self.unsplit(),
            WorkspaceAction::FocusGroup { group, leaving } => self.focus_group(*group, *leaving),
            WorkspaceAction::SetRatio { permille } => {
                self.state.ratio_permille =
                    (*permille).clamp(RATIO_MIN_PERMILLE, RATIO_MAX_PERMILLE);
                Ok(())
            }
            WorkspaceAction::Back { leaving } => self.walk(Direction::Previous, *leaving),
            WorkspaceAction::Forward { leaving } => self.walk(Direction::Next, *leaving),
            WorkspaceAction::Renamed { from, to } => {
                self.renamed(from, to);
                Ok(())
            }
            WorkspaceAction::Deleted { path } => {
                self.deleted(path);
                Ok(())
            }
        }
    }

    // ── Поиск и мелкие операции ─────────────────────────────────────────────

    fn find(&self, tab: TabId) -> Option<(usize, usize)> {
        self.state.groups.iter().enumerate().find_map(|(g, group)| {
            group
                .tabs
                .iter()
                .position(|t| t.id == tab)
                .map(|index| (g, index))
        })
    }

    fn find_path_in(&self, group: usize, path: &str) -> Option<TabId> {
        self.state
            .groups
            .get(group)?
            .tabs
            .iter()
            .find(|t| t.path == path)
            .map(|t| t.id)
    }

    /// Группа, где уже открыта доска (путь доски уникален на всё окно).
    fn find_board(&self, path: &str) -> Option<(usize, TabId)> {
        self.state.groups.iter().enumerate().find_map(|(g, group)| {
            group
                .tabs
                .iter()
                .find(|t| t.kind == FileKind::Board && t.path == path)
                .map(|t| (g, t.id))
        })
    }

    fn active_index(&self) -> usize {
        self.state
            .active_group
            .index()
            .min(self.state.groups.len() - 1)
    }

    fn tab(&self, group: usize, tab: TabId) -> Option<&Tab> {
        self.state
            .groups
            .get(group)?
            .tabs
            .iter()
            .find(|t| t.id == tab)
    }

    fn active_path(&self, group: usize) -> Option<String> {
        let group = self.state.groups.get(group)?;
        let active = group.active?;
        group
            .tabs
            .iter()
            .find(|t| t.id == active)
            .map(|t| t.path.clone())
    }

    fn new_tab_id(&mut self) -> TabId {
        let id = TabId(self.state.next_tab);
        self.state.next_tab += 1;
        id
    }

    fn set_focus(&mut self, group: usize, tab: TabId, place: Option<TextPlace>) {
        if let Some(found) = self.tab(group, tab) {
            self.focus = Some(Focus {
                group: GroupSlot::of_index(group),
                tab,
                path: found.path.clone(),
                kind: found.kind,
                place,
            });
        }
    }

    /// Фокус на активной вкладке активной группы, если она есть.
    fn focus_active(&mut self) {
        let group = self.active_index();
        if let Some(tab) = self.state.groups[group].active {
            self.set_focus(group, tab, None);
        }
    }

    /// Делает вкладку активной в группе, без истории.
    fn make_active(group: &mut Group, tab: TabId) {
        group.active = Some(tab);
        group.recent.retain(|id| *id != tab);
        group.recent.insert(0, tab);
    }

    /// Переход пользователя на вкладку: делает её активной, переводит каретку
    /// в её группу и пишет запись в историю группы. `leaving` — место каретки
    /// на заметке, которую оставляют в *активной* группе.
    fn navigate(
        &mut self,
        group: usize,
        tab: TabId,
        leaving: Option<TextPlace>,
        target: Option<TextPlace>,
    ) {
        let Some(to_path) = self.tab(group, tab).map(|t| t.path.clone()) else {
            return;
        };
        let current = self.active_index();
        let mut leaving = leaving;
        if group != current {
            // Каретка уходит из другой группы: место запоминает её история.
            if let Some(from) = self.active_path(current) {
                self.state.groups[current]
                    .history
                    .note_leaving(&from, leaving);
            }
            leaving = None;
        }
        let from = self.active_path(group);
        let target_group = &mut self.state.groups[group];
        target_group.history.visit(
            from.as_deref(),
            leaving,
            HistoryEntry {
                path: to_path,
                place: target,
            },
        );
        Self::make_active(target_group, tab);
        self.state.active_group = GroupSlot::of_index(group);
        self.set_focus(group, tab, target);
    }

    // ── Открытие и обход ────────────────────────────────────────────────────

    fn open(
        &mut self,
        path: &str,
        target: Option<TextPlace>,
        leaving: Option<TextPlace>,
    ) -> Result<(), Rejection> {
        let kind = self
            .facts
            .note(path)
            .map(|fact| fact.kind)
            .filter(|kind| is_openable(*kind))
            .ok_or(Rejection::NotFound)?;

        let existing = if kind == FileKind::Board {
            self.find_board(path)
        } else {
            let group = self.active_index();
            self.find_path_in(group, path).map(|id| (group, id))
        };
        if let Some((group, id)) = existing {
            self.navigate(group, id, leaving, target);
            return Ok(());
        }

        let group = self.active_index();
        let id = self.new_tab_id();
        self.state.groups[group].tabs.push(Tab {
            id,
            path: path.to_owned(),
            kind,
        });
        self.navigate(group, id, leaving, target);
        Ok(())
    }

    fn cycle(&mut self, direction: Direction, leaving: Option<TextPlace>) {
        let group = self.active_index();
        let tabs = &self.state.groups[group].tabs;
        if tabs.len() < 2 {
            return;
        }
        let active = self.state.groups[group].active;
        let position = tabs.iter().position(|t| Some(t.id) == active).unwrap_or(0);
        let next = match direction {
            Direction::Next => (position + 1) % tabs.len(),
            Direction::Previous => (position + tabs.len() - 1) % tabs.len(),
        };
        let id = tabs[next].id;
        self.navigate(group, id, leaving, None);
    }

    /// «Назад» и «Вперёд»: указатель истории активной группы движется, записи
    /// не добавляются (FR-011…FR-014).
    fn walk(&mut self, direction: Direction, leaving: Option<TextPlace>) -> Result<(), Rejection> {
        let facts = self.facts;
        let group = self.active_index();
        let empty = self.state.groups[group].active.is_none();
        let index = self.state.groups[group]
            .history
            .step(direction, empty, |path| {
                facts.note(path).is_some_and(|f| is_openable(f.kind))
            })
            .ok_or(Rejection::NoHistory)?;
        let entry = self.state.groups[group].history.entries[index].clone();
        let kind = facts
            .note(&entry.path)
            .map(|f| f.kind)
            .ok_or(Rejection::NoHistory)?;

        if let Some(from) = self.active_path(group) {
            self.state.groups[group]
                .history
                .note_leaving(&from, leaving);
        }
        self.state.groups[group].history.cursor = index;

        // Открывает запись без новой записи в историю.
        let (target_group, id) = if kind == FileKind::Board {
            match self.find_board(&entry.path) {
                Some(found) => found,
                None => (group, self.push_tab(group, &entry.path, kind)),
            }
        } else {
            match self.find_path_in(group, &entry.path) {
                Some(id) => (group, id),
                None => (group, self.push_tab(group, &entry.path, kind)),
            }
        };
        Self::make_active(&mut self.state.groups[target_group], id);
        self.state.active_group = GroupSlot::of_index(target_group);
        self.set_focus(target_group, id, entry.place);
        Ok(())
    }

    fn push_tab(&mut self, group: usize, path: &str, kind: FileKind) -> TabId {
        let id = self.new_tab_id();
        self.state.groups[group].tabs.push(Tab {
            id,
            path: path.to_owned(),
            kind,
        });
        id
    }

    // ── Закрытие ────────────────────────────────────────────────────────────

    /// Причина, по которой вкладку нельзя закрыть, решается только по фактам
    /// (FR-005, SC-004).
    fn refusal_for(&self, tab: &Tab, board_unsaved: bool) -> Option<RefusalReason> {
        if tab.kind == FileKind::Board {
            return board_unsaved.then_some(RefusalReason::Unsaved);
        }
        let fact = self.facts.note(&tab.path)?;
        if fact.conflict {
            Some(RefusalReason::Conflict)
        } else if fact.unsaved {
            Some(RefusalReason::Unsaved)
        } else {
            None
        }
    }

    /// Закрывает вкладку с проверкой. Если тот же путь открыт в другой группе,
    /// проверки нет — документ остаётся открытым (FR-017); в слитой подаче
    /// наоборот: закрываются обе вкладки пути, и проверка обязательна
    /// (research.md §10).
    fn close_checked(
        &mut self,
        group: usize,
        tab: TabId,
        board_unsaved: bool,
        activate_on_conflict: bool,
    ) {
        let Some(found) = self.tab(group, tab).cloned() else {
            return;
        };
        let merged = is_merged(&self.state, self.facts);
        let twin = if found.kind == FileKind::Note && self.state.groups.len() == 2 {
            self.find_path_in(1 - group, &found.path)
        } else {
            None
        };

        if twin.is_none() || merged {
            if let Some(reason) = self.refusal_for(&found, board_unsaved) {
                self.refused.push(CloseRefusal {
                    tab,
                    path: found.path.clone(),
                    reason,
                });
                if reason == RefusalReason::Conflict && activate_on_conflict {
                    self.navigate(group, tab, None, None);
                }
                return;
            }
        }

        let mut ids = vec![tab];
        if merged {
            ids.extend(twin);
        }
        self.remove_tabs(&ids, true);
    }

    fn close_others(&mut self, tab: TabId) -> Result<(), Rejection> {
        let (group, _) = self.find(tab).ok_or(Rejection::UnknownTab)?;
        self.navigate(group, tab, None, None);
        let others: Vec<TabId> = self.state.groups[group]
            .tabs
            .iter()
            .map(|t| t.id)
            .filter(|id| *id != tab)
            .collect();
        for id in others {
            // Вкладка могла уйти вместе с двойником в слитой подаче.
            if let Some((g, _)) = self.find(id) {
                self.close_checked(g, id, false, false);
            }
        }
        if let Some((g, _)) = self.find(tab) {
            self.set_focus(g, tab, None);
        }
        Ok(())
    }

    /// Убирает вкладки. Для закрытой активной вкладки активной становится
    /// предыдущая по давности (FR-006) — с записью в историю группы, если
    /// `record`. Опустевшая группа схлопывается (FR-019).
    fn remove_tabs(&mut self, ids: &[TabId], record: bool) {
        let before = {
            let group = self.active_index();
            self.state.groups[group].active
        };
        for &id in ids {
            let Some((g, index)) = self.find(id) else {
                continue;
            };
            let group = &mut self.state.groups[g];
            let removed = group.tabs.remove(index);
            group.recent.retain(|r| *r != id);
            self.closed.push(id);
            if group.active == Some(id) {
                group.active = group.recent.first().copied();
                if let (true, Some(next)) = (record, group.active) {
                    if let Some(to) = group.tabs.iter().find(|t| t.id == next) {
                        let entry = HistoryEntry {
                            path: to.path.clone(),
                            place: None,
                        };
                        group.history.visit(Some(&removed.path), None, entry);
                    }
                }
            }
        }
        self.collapse();
        let after = {
            let group = self.active_index();
            self.state.groups[group].active
        };
        if after != before {
            self.focus_active();
        }
    }

    /// Опустевшая группа убирается; оставшаяся — первая (FR-019).
    fn collapse(&mut self) {
        if self.state.groups.len() == 2 {
            if self.state.groups[1].tabs.is_empty() {
                self.state.groups.pop();
                self.state.active_group = GroupSlot::First;
            } else if self.state.groups[0].tabs.is_empty() {
                self.state.groups.remove(0);
                self.state.active_group = GroupSlot::First;
            }
        }
        if self.state.active_group.index() >= self.state.groups.len() {
            self.state.active_group = GroupSlot::First;
        }
    }

    // ── Порядок и группы ────────────────────────────────────────────────────

    fn move_tab(&mut self, tab: TabId, to: GroupSlot, index: usize) -> Result<(), Rejection> {
        let (from, position) = self.find(tab).ok_or(Rejection::UnknownTab)?;
        let dest = to.index();
        if dest >= self.state.groups.len() {
            return Err(Rejection::SingleGroup);
        }

        if dest == from {
            let group = &mut self.state.groups[from];
            let moved = group.tabs.remove(position);
            group.tabs.insert(index.min(group.tabs.len()), moved);
            return Ok(());
        }

        let moved = self.state.groups[from].tabs.remove(position);
        let source = &mut self.state.groups[from];
        source.recent.retain(|id| *id != tab);
        if source.active == Some(tab) {
            source.active = source.recent.first().copied();
            if let Some(next) = source.active {
                if let Some(next_tab) = source.tabs.iter().find(|t| t.id == next) {
                    let entry = HistoryEntry {
                        path: next_tab.path.clone(),
                        place: None,
                    };
                    source.history.visit(Some(&moved.path), None, entry);
                }
            }
        }

        // Заметка, уже открытая в той группе, не дублируется: активируется
        // существующая вкладка, а переносимая исчезает.
        let existing = if moved.kind == FileKind::Note {
            self.find_path_in(dest, &moved.path)
        } else {
            None
        };
        match existing {
            Some(existing) => {
                self.closed.push(tab);
                self.navigate(dest, existing, None, None);
            }
            None => {
                let group = &mut self.state.groups[dest];
                group.tabs.insert(index.min(group.tabs.len()), moved);
                self.navigate(dest, tab, None, None);
            }
        }
        self.collapse();
        Ok(())
    }

    fn split(&mut self, leaving: Option<TextPlace>) -> Result<(), Rejection> {
        if self.facts.width_px < SPLIT_MIN_WIDTH_PX {
            return Err(Rejection::TooNarrow);
        }
        if self.state.groups.len() != 1 {
            return Err(Rejection::NothingToSplit);
        }
        let group = &self.state.groups[0];
        let active = group
            .active
            .and_then(|id| group.tabs.iter().find(|t| t.id == id))
            .cloned()
            .ok_or(Rejection::NothingToSplit)?;
        if active.kind == FileKind::Board && group.tabs.len() == 1 {
            return Err(Rejection::NothingToSplit);
        }

        self.state.groups[0]
            .history
            .note_leaving(&active.path, leaving);

        let second_tab = if active.kind == FileKind::Board {
            // Доска существует в одном экземпляре: переносится, а не
            // дублируется (research.md §8).
            self.remove_from_first_group(active.id);
            active
        } else {
            Tab {
                id: self.new_tab_id(),
                path: active.path,
                kind: active.kind,
            }
        };
        let id = second_tab.id;
        let mut second = Group {
            tabs: vec![second_tab],
            active: Some(id),
            recent: vec![id],
            history: Default::default(),
        };
        second.history.visit(
            None,
            None,
            HistoryEntry {
                path: second.tabs[0].path.clone(),
                place: None,
            },
        );
        self.state.groups.push(second);
        self.state.active_group = GroupSlot::Second;
        self.set_focus(1, id, None);
        Ok(())
    }

    /// Убирает вкладку из первой группы без `closed` (она переезжает).
    fn remove_from_first_group(&mut self, tab: TabId) {
        let group = &mut self.state.groups[0];
        let Some(index) = group.tabs.iter().position(|t| t.id == tab) else {
            return;
        };
        let removed = group.tabs.remove(index);
        group.recent.retain(|id| *id != tab);
        group.active = group.recent.first().copied();
        if let Some(next) = group.active {
            if let Some(next_tab) = group.tabs.iter().find(|t| t.id == next) {
                let entry = HistoryEntry {
                    path: next_tab.path.clone(),
                    place: None,
                };
                group.history.visit(Some(&removed.path), None, entry);
            }
        }
    }

    /// Вторая группа сливается с первой: вкладки дописываются в конец без
    /// дубликатов (FR-015). История первой группы остаётся, второй — уходит.
    fn unsplit(&mut self) -> Result<(), Rejection> {
        if self.state.groups.len() < 2 {
            return Err(Rejection::SingleGroup);
        }
        let active_slot = self.state.active_group;
        let second = self.state.groups.pop().expect("две группы");
        let first = &mut self.state.groups[0];

        let mut twin_of: Vec<(TabId, TabId)> = Vec::new();
        for tab in second.tabs {
            match first.tabs.iter().find(|t| t.path == tab.path) {
                Some(twin) => {
                    twin_of.push((tab.id, twin.id));
                    self.closed.push(tab.id);
                }
                None => first.tabs.push(tab),
            }
        }
        let wanted = match active_slot {
            GroupSlot::First => first.active,
            GroupSlot::Second => second.active.map(|id| {
                twin_of
                    .iter()
                    .find(|(dropped, _)| *dropped == id)
                    .map_or(id, |(_, twin)| *twin)
            }),
        }
        .or_else(|| first.tabs.first().map(|t| t.id));

        let mut recent: Vec<TabId> = Vec::new();
        recent.extend(wanted);
        recent.extend(first.recent.iter().copied());
        recent.extend(second.recent.iter().copied());
        first.recent = recent;
        first.active = wanted;
        repair_recent(first);

        self.state.active_group = GroupSlot::First;
        self.focus_active();
        Ok(())
    }

    fn focus_group(
        &mut self,
        group: GroupSlot,
        leaving: Option<TextPlace>,
    ) -> Result<(), Rejection> {
        if group.index() >= self.state.groups.len() {
            return Err(Rejection::SingleGroup);
        }
        let current = self.active_index();
        if group.index() != current {
            if let Some(from) = self.active_path(current) {
                self.state.groups[current]
                    .history
                    .note_leaving(&from, leaving);
            }
            self.state.active_group = group;
        }
        self.focus_active();
        Ok(())
    }

    // ── События vault ───────────────────────────────────────────────────────

    fn renamed(&mut self, from: &str, to: &str) {
        for group in &mut self.state.groups {
            for tab in &mut group.tabs {
                if tab.path == from {
                    tab.path = to.to_owned();
                }
            }
            group.history.rename(from, to);
        }
        // Переименование на путь, который уже открыт, не должно оставить двух
        // вкладок одного пути в группе (и двух досок в окне).
        let mut duplicates: Vec<TabId> = Vec::new();
        let mut boards: Vec<&str> = Vec::new();
        for group in &self.state.groups {
            let mut seen: Vec<&str> = Vec::new();
            for tab in &group.tabs {
                let board_seen = tab.kind == FileKind::Board && boards.contains(&tab.path.as_str());
                if seen.contains(&tab.path.as_str()) || board_seen {
                    duplicates.push(tab.id);
                } else {
                    seen.push(&tab.path);
                    if tab.kind == FileKind::Board {
                        boards.push(&tab.path);
                    }
                }
            }
        }
        if !duplicates.is_empty() {
            self.remove_tabs(&duplicates, false);
        }
    }

    /// Удалённая заметка закрывается во всех группах, кроме случая конфликта:
    /// удаление с несохранёнными правками оставляет вкладку (research.md §7).
    fn deleted(&mut self, path: &str) {
        if self.facts.note(path).is_some_and(|fact| fact.conflict) {
            return;
        }
        for group in &mut self.state.groups {
            group.history.purge(path);
        }
        let ids: Vec<TabId> = self
            .state
            .groups
            .iter()
            .flat_map(|group| group.tabs.iter())
            .filter(|tab| tab.path == path)
            .map(|tab| tab.id)
            .collect();
        if !ids.is_empty() {
            self.remove_tabs(&ids, false);
        }
    }
}

/// Приводит `recent` к перестановке идентификаторов вкладок группы с активной
/// впереди; недостающие дописываются в порядке полосы.
pub(crate) fn repair_recent(group: &mut Group) {
    let mut recent: Vec<TabId> = Vec::with_capacity(group.tabs.len());
    for id in group.recent.iter().copied() {
        if group.tabs.iter().any(|t| t.id == id) && !recent.contains(&id) {
            recent.push(id);
        }
    }
    for tab in &group.tabs {
        if !recent.contains(&tab.id) {
            recent.push(tab.id);
        }
    }
    if let Some(active) = group.active {
        if let Some(position) = recent.iter().position(|id| *id == active) {
            recent.remove(position);
            recent.insert(0, active);
        }
    }
    group.recent = recent;
}

/// Проверка инвариантов data-model.md; текст — что именно нарушено. Нужна
/// тестам и отладке: переходы строят корректное состояние по построению.
pub fn check_invariants(state: &WorkspaceState) -> Result<(), String> {
    if !(1..=2).contains(&state.groups.len()) {
        return Err(format!("групп: {}", state.groups.len()));
    }
    if state.groups.len() == 2 && state.groups.iter().any(|g| g.tabs.is_empty()) {
        return Err("пустая группа при двух".into());
    }
    if state.active_group.index() >= state.groups.len() {
        return Err("активная группа не существует".into());
    }
    if !(RATIO_MIN_PERMILLE..=RATIO_MAX_PERMILLE).contains(&state.ratio_permille) {
        return Err(format!("доля {}", state.ratio_permille));
    }
    let mut ids: Vec<TabId> = Vec::new();
    let mut boards: Vec<&str> = Vec::new();
    for (index, group) in state.groups.iter().enumerate() {
        let mut paths: Vec<&str> = Vec::new();
        for tab in &group.tabs {
            if ids.contains(&tab.id) {
                return Err(format!("повтор {:?}", tab.id));
            }
            if tab.id.0 >= state.next_tab {
                return Err(format!("{:?} не меньше next_tab", tab.id));
            }
            ids.push(tab.id);
            if paths.contains(&tab.path.as_str()) {
                return Err(format!("группа {index}: повтор пути {}", tab.path));
            }
            paths.push(&tab.path);
            if tab.kind == FileKind::Board {
                if boards.contains(&tab.path.as_str()) {
                    return Err(format!("доска {} дважды", tab.path));
                }
                boards.push(&tab.path);
            }
        }
        match group.active {
            Some(active) if !group.tabs.iter().any(|t| t.id == active) => {
                return Err(format!("группа {index}: активной вкладки нет"));
            }
            None if !group.tabs.is_empty() => {
                return Err(format!("группа {index}: есть вкладки, нет активной"));
            }
            _ => {}
        }
        if group.recent.len() != group.tabs.len()
            || group.recent.first().copied() != group.active
            || group.tabs.iter().any(|tab| !group.recent.contains(&tab.id))
        {
            return Err(format!("группа {index}: recent не согласован"));
        }
        if group.history.entries.len() > super::HISTORY_LIMIT {
            return Err(format!("группа {index}: история длиннее предела"));
        }
        if !group.history.entries.is_empty() && group.history.cursor >= group.history.entries.len()
        {
            return Err(format!("группа {index}: указатель истории вне записей"));
        }
    }
    Ok(())
}
