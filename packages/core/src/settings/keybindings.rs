//! Сочетания клавиш команд приложения.
//!
//! Здесь — только правила: какие команды вообще существуют, какое сочетание у
//! каждой по умолчанию, что считается допустимым сочетанием и как оно
//! записывается словами. Хранение — уровня vault
//! ([`crate::vault::vault_settings::VaultOverrides`]), и сам файл этот модуль
//! не трогает.
//!
//! # Почему структура, а не строка
//!
//! Наружу сочетание уезжает разобранным — четыре поля, а не `"Mod+Shift+F"`.
//! Сопоставление с нажатием происходит в интерфейсе на каждое нажатие клавиши:
//! ходить за ним в ядро через границу процесса нельзя, а разбирать строку на
//! той стороне значило бы завести вторую копию правила разбора (принцип I).
//! Со структурой интерфейсу остаётся сравнение полей на равенство — это не
//! правило, а сверка.
//!
//! Текстовая форма при этом существует: в файле настроек лежит именно она,
//! потому что файл читают и правят глазами.

use serde::{Deserialize, Serialize};

/// Команда приложения, которой можно назначить сочетание.
///
/// Перечисление, а не свободная строка: команда, которой нет, не должна быть
/// выразима — ни в файле пользователя, ни в интерфейсе (принцип IV). Разбор
/// незнакомого имени из файла поэтому даёт `None`, а не «команду с таким
/// именем».
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "kebab-case")]
pub enum CommandId {
    /// Палитра команд.
    CommandPalette,
    /// Расширенный поиск по vault.
    Search,
    /// Галерея заготовок.
    Templates,
    /// Создать заметку в дереве.
    NewFile,
    /// Создать доску в дереве.
    NewBoard,
    /// Меню вставки элементов Typst в редакторе.
    InsertMenu,
    /// Параметры элемента Typst под кареткой (фича 008, FR-006).
    ElementParams,
    /// Переключение между рабочей областью и графом связей.
    ToggleGraph,
    /// Пересобрать предпросмотр.
    RebuildPreview,
}

/// Все команды в том порядке, в котором их показывает панель настроек.
///
/// Порядок задан здесь, а не в интерфейсе: список команд — знание ядра, и
/// вторая его копия разошлась бы при добавлении первой же новой команды.
pub const COMMANDS: &[CommandId] = &[
    CommandId::CommandPalette,
    CommandId::Search,
    CommandId::Templates,
    CommandId::NewFile,
    CommandId::NewBoard,
    CommandId::InsertMenu,
    CommandId::ElementParams,
    CommandId::ToggleGraph,
    CommandId::RebuildPreview,
];

impl CommandId {
    /// Имя команды в файле настроек.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommandPalette => "command-palette",
            Self::Search => "search",
            Self::Templates => "templates",
            Self::NewFile => "new-file",
            Self::NewBoard => "new-board",
            Self::InsertMenu => "insert-menu",
            Self::ElementParams => "element-params",
            Self::ToggleGraph => "toggle-graph",
            Self::RebuildPreview => "rebuild-preview",
        }
    }

    /// Разбирает имя из файла. `None` — команды с таким именем нет.
    pub fn parse(text: &str) -> Option<Self> {
        COMMANDS.iter().copied().find(|id| id.as_str() == text)
    }

    /// Подпись команды для панели настроек.
    ///
    /// В ядре по той же причине, что и список: подпись обязана существовать для
    /// каждой команды, и `match` без ветки не соберётся (принцип IV).
    pub fn title(self) -> &'static str {
        match self {
            Self::CommandPalette => "Палитра команд",
            Self::Search => "Расширенный поиск",
            Self::Templates => "Шаблоны и сниппеты",
            Self::NewFile => "Создать заметку",
            Self::NewBoard => "Создать доску",
            Self::InsertMenu => "Вставить элемент Typst",
            Self::ElementParams => "Параметры элемента",
            Self::ToggleGraph => "Граф связей",
            Self::RebuildPreview => "Пересобрать предпросмотр",
        }
    }

    /// Сочетание по умолчанию.
    ///
    /// Значения те же, что были зашиты в интерфейсе до появления настройки, —
    /// кроме двух команд, у которых сочетания не было вовсе.
    pub fn default_shortcut(self) -> Shortcut {
        let (shift, alt, key) = match self {
            Self::CommandPalette => (false, false, "k"),
            Self::Search => (true, false, "f"),
            Self::Templates => (true, false, "t"),
            Self::NewFile => (false, false, "n"),
            Self::NewBoard => (true, false, "n"),
            Self::InsertMenu => (false, false, "e"),
            // Пара к меню вставки: «вставить элемент» и «параметры элемента» —
            // один жест с Shift (research.md §9).
            Self::ElementParams => (true, false, "e"),
            // Сочетания не было: команда вызывалась только кнопкой в шапке.
            Self::ToggleGraph => (true, false, "g"),
            // `Mod+R` занят перезагрузкой страницы в браузере, `Mod+Shift+R` —
            // перезагрузкой без кэша. Обе сборки обязаны вести себя одинаково
            // (принцип II), поэтому берётся свободное на всех.
            Self::RebuildPreview => (true, false, "b"),
        };
        Shortcut {
            mod_key: true,
            shift,
            alt,
            key: key.to_string(),
        }
    }
}

/// Сочетание клавиш: модификаторы и одна клавиша.
///
/// `mod_key` — главный модификатор платформы: ⌘ на macOS, Ctrl на остальных.
/// Одно поле, а не два: одно и то же сочетание обязано работать на обеих
/// платформах, и различать их в хранимом значении значило бы сделать файл
/// настроек непереносимым между машинами пользователя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Shortcut {
    pub mod_key: bool,
    pub shift: bool,
    pub alt: bool,
    /// Клавиша так, как её называет `KeyboardEvent.key`, приведённая к нижнему
    /// регистру: `"k"`, `"f1"`, `"arrowup"`.
    pub key: String,
}

/// Почему сочетание не годится.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ShortcutProblem {
    /// Ни одного модификатора: такое сочетание срабатывало бы при обычном
    /// наборе текста.
    #[error("сочетание без модификатора срабатывало бы при наборе текста")]
    NoModifier,
    /// Клавиша занята самим интерфейсом и переназначению не подлежит.
    #[error("эта клавиша занята интерфейсом")]
    ReservedKey,
    /// Нажат только модификатор — самой клавиши нет.
    #[error("нужно нажать клавишу вместе с модификатором")]
    NoKey,
}

/// Клавиши, которые нельзя назначить командам.
///
/// Escape закрывает любое наложение, Enter и Tab работают в каждом списке и
/// поле ввода: команда, перехватившая их глобально, сломала бы интерфейс
/// целиком, а не только себя.
const RESERVED_KEYS: &[&str] = &[
    "escape", "enter", "tab", "backspace", "delete", " ", "arrowup", "arrowdown", "arrowleft",
    "arrowright",
];

/// Имена клавиш-модификаторов: сами по себе сочетанием они не являются.
const MODIFIER_KEYS: &[&str] = &[
    "control", "shift", "alt", "meta", "os", "altgraph", "capslock", "dead",
];

impl Shortcut {
    /// Приводит нажатие к хранимому виду.
    ///
    /// Вход — ровно то, что дал `KeyboardEvent`: три флага и имя клавиши.
    /// Приведение к нижнему регистру именно здесь, а не в интерфейсе: иначе
    /// `Shift+F` и `Shift+f` стали бы разными сочетаниями в зависимости от
    /// того, как браузер назвал клавишу.
    pub fn normalize(mod_key: bool, shift: bool, alt: bool, key: &str) -> Self {
        Self {
            mod_key,
            shift,
            alt,
            key: key.to_lowercase(),
        }
    }

    /// Годится ли сочетание. `None` — годится.
    pub fn problem(&self) -> Option<ShortcutProblem> {
        if self.key.is_empty() || MODIFIER_KEYS.contains(&self.key.as_str()) {
            return Some(ShortcutProblem::NoKey);
        }
        if RESERVED_KEYS.contains(&self.key.as_str()) {
            return Some(ShortcutProblem::ReservedKey);
        }
        // Shift сам по себе не считается: `Shift+F` — это просто «F».
        if !self.mod_key && !self.alt {
            return Some(ShortcutProblem::NoModifier);
        }
        None
    }

    /// Текстовая форма для файла настроек: `Mod+Shift+F`.
    pub fn to_text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.mod_key {
            parts.push("Mod".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        parts.push(display_key(&self.key));
        parts.join("+")
    }

    /// Разбирает текстовую форму. `None` — строка не является сочетанием.
    ///
    /// Синонимы модификаторов принимаются намеренно: пользователь, правящий
    /// файл руками, напишет `Cmd` или `Ctrl` раньше, чем догадается про `Mod`.
    pub fn parse(text: &str) -> Option<Self> {
        let mut mod_key = false;
        let mut shift = false;
        let mut alt = false;
        let mut key: Option<String> = None;

        for part in text.split('+') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.to_lowercase().as_str() {
                "mod" | "cmd" | "command" | "ctrl" | "control" | "meta" | "super" => mod_key = true,
                "shift" => shift = true,
                "alt" | "option" | "opt" => alt = true,
                other => {
                    // Две клавиши в одном сочетании — это не сочетание, а
                    // опечатка: молча взять последнюю значило бы применить не
                    // то, что написано.
                    if key.is_some() {
                        return None;
                    }
                    key = Some(other.to_string());
                }
            }
        }

        Some(Self {
            mod_key,
            shift,
            alt,
            key: key?,
        })
    }

    /// Имя физической клавиши (`KeyboardEvent.code`), если оно определено.
    ///
    /// Нужно из-за раскладки: при кириллической раскладке `event.key` для той
    /// же клавиши даёт `"л"`, а не `"k"`, и сочетание, сверяемое только по
    /// `key`, переставало бы работать ровно тогда, когда пользователь пишет
    /// по-русски, — то есть всегда в этом приложении.
    ///
    /// Вывод живёт здесь, а не в обработчике нажатий: «`k` — это `KeyK`» —
    /// правило, и его вторая копия в TypeScript разошлась бы с этой (принцип I).
    /// `None` — у клавиши нет устойчивого имени (`F1`, `Home`); такие сверяются
    /// только по `key`, где раскладка и не мешает.
    pub fn code(&self) -> Option<String> {
        let mut chars = self.key.chars();
        let (first, rest) = (chars.next()?, chars.next());
        if rest.is_some() {
            return None;
        }
        if first.is_ascii_alphabetic() {
            return Some(format!("Key{}", first.to_ascii_uppercase()));
        }
        if first.is_ascii_digit() {
            return Some(format!("Digit{first}"));
        }
        None
    }

    /// Как сочетание показывается пользователю.
    ///
    /// `mac` приходит снаружи: платформы ядро не знает ни на одном таргете, и
    /// заводить её здесь значило бы завести платформенный шов ради подписи.
    pub fn display(&self, mac: bool) -> String {
        if mac {
            let mut text = String::new();
            if self.alt {
                text.push('⌥');
            }
            if self.shift {
                text.push('⇧');
            }
            if self.mod_key {
                text.push('⌘');
            }
            text.push_str(&display_key(&self.key));
            return text;
        }

        let mut parts: Vec<String> = Vec::new();
        if self.mod_key {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        parts.push(display_key(&self.key));
        parts.join("+")
    }
}

/// Имя клавиши в подписи: одиночная буква — заглавной, всё прочее — как есть.
fn display_key(key: &str) -> String {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) if key.chars().count() == 1 => first.to_uppercase().to_string(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Сочетание, назначенное команде пользователем (запись в файле настроек).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct KeybindingEntry {
    pub command: CommandId,
    /// `None` — сочетание снято: команда остаётся, но клавишами не вызывается.
    /// Отличать от «не задано» обязательно, иначе снятое сочетание вернулось бы
    /// умолчанием при следующем чтении файла.
    pub shortcut: Option<Shortcut>,
}

/// Действующее сочетание одной команды — всё, что нужно и панели настроек, и
/// обработчику нажатий.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ResolvedBinding {
    pub command: CommandId,
    pub title: String,
    /// `None` — сочетание снято пользователем.
    pub shortcut: Option<Shortcut>,
    /// Подпись сочетания; пустая строка, если оно снято.
    pub display: String,
    /// Имя физической клавиши для сверки с `KeyboardEvent.code`; пустая строка,
    /// если у клавиши устойчивого имени нет. См. [`Shortcut::code`].
    pub code: String,
    /// Совпадает ли действующее значение с умолчанием.
    pub is_default: bool,
    /// Команды, которым назначено то же сочетание.
    ///
    /// Конфликт не запрещается, а показывается: запретить его значило бы
    /// заставить пользователя расставлять сочетания в определённом порядке.
    /// Разрешает конфликт он сам, видя обе команды.
    pub conflicts_with: Vec<CommandId>,
}

/// Действующие сочетания всех команд: умолчание, если пользователь не задал
/// своего.
pub fn resolve(entries: &[KeybindingEntry], mac: bool) -> Vec<ResolvedBinding> {
    let effective: Vec<(CommandId, Option<Shortcut>)> = COMMANDS
        .iter()
        .map(|&command| {
            let assigned = entries.iter().find(|entry| entry.command == command);
            let shortcut = match assigned {
                Some(entry) => entry.shortcut.clone(),
                None => Some(command.default_shortcut()),
            };
            (command, shortcut)
        })
        .collect();

    effective
        .iter()
        .map(|(command, shortcut)| ResolvedBinding {
            command: *command,
            // `String`, а не `&'static str`: типы, уезжающие через `tsify`,
            // обязаны владеть своими данными.
            title: command.title().to_string(),
            display: shortcut
                .as_ref()
                .map(|value| value.display(mac))
                .unwrap_or_default(),
            code: shortcut
                .as_ref()
                .and_then(|value| value.code())
                .unwrap_or_default(),
            is_default: shortcut.as_ref() == Some(&command.default_shortcut()),
            conflicts_with: shortcut
                .as_ref()
                .map(|value| {
                    effective
                        .iter()
                        .filter(|(other, other_shortcut)| {
                            other != command && other_shortcut.as_ref() == Some(value)
                        })
                        .map(|(other, _)| *other)
                        .collect()
                })
                .unwrap_or_default(),
            shortcut: shortcut.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_a_title_and_a_default() {
        // Принцип IV: новая команда не может проскочить без подписи и
        // умолчания — `match` без ветки не соберётся. Тест закрепляет, что
        // список и сами значения не разошлись.
        for &command in COMMANDS {
            assert!(!command.title().is_empty(), "{command:?} без подписи");
            assert!(
                command.default_shortcut().problem().is_none(),
                "{command:?}: умолчание само по себе недопустимо"
            );
        }
    }

    #[test]
    fn defaults_do_not_collide_with_each_other() {
        // Умолчания, конфликтующие между собой, означали бы, что приложение из
        // коробки поставляется со сломанной командой.
        let resolved = resolve(&[], false);
        for binding in &resolved {
            assert!(
                binding.conflicts_with.is_empty(),
                "{:?} конфликтует с {:?}",
                binding.command,
                binding.conflicts_with
            );
        }
    }

    #[test]
    fn a_shortcut_survives_the_text_round_trip() {
        for &command in COMMANDS {
            let shortcut = command.default_shortcut();
            let text = shortcut.to_text();
            assert_eq!(
                Shortcut::parse(&text).as_ref(),
                Some(&shortcut),
                "не пережило {text}"
            );
        }
    }

    #[test]
    fn modifier_synonyms_are_accepted_from_a_hand_written_file() {
        let expected = Shortcut::normalize(true, true, false, "f");
        for text in ["Mod+Shift+F", "Cmd+Shift+f", "ctrl+shift+F", "Control+Shift+F"] {
            assert_eq!(Shortcut::parse(text).as_ref(), Some(&expected), "{text}");
        }
    }

    #[test]
    fn two_keys_in_one_shortcut_is_a_typo_not_a_shortcut() {
        // Взять последнюю клавишу значило бы применить не то, что написано.
        assert_eq!(Shortcut::parse("Mod+K+L"), None);
        // Один модификатор без клавиши — тоже не сочетание.
        assert_eq!(Shortcut::parse("Mod+Shift"), None);
    }

    #[test]
    fn a_shortcut_without_a_modifier_is_rejected() {
        // Иначе команда срабатывала бы при обычном наборе текста.
        assert_eq!(
            Shortcut::normalize(false, false, false, "k").problem(),
            Some(ShortcutProblem::NoModifier)
        );
        // Shift сам по себе модификатором не считается: `Shift+F` — это «F».
        assert_eq!(
            Shortcut::normalize(false, true, false, "f").problem(),
            Some(ShortcutProblem::NoModifier)
        );
        // Alt считается.
        assert_eq!(Shortcut::normalize(false, false, true, "f").problem(), None);
    }

    #[test]
    fn keys_the_interface_lives_on_cannot_be_taken() {
        for key in ["escape", "enter", "tab", "arrowup"] {
            assert_eq!(
                Shortcut::normalize(true, false, false, key).problem(),
                Some(ShortcutProblem::ReservedKey),
                "{key}"
            );
        }
    }

    #[test]
    fn a_modifier_alone_is_not_a_shortcut() {
        for key in ["Shift", "Control", "Meta", ""] {
            assert_eq!(
                Shortcut::normalize(true, false, false, key).problem(),
                Some(ShortcutProblem::NoKey),
                "{key}"
            );
        }
    }

    #[test]
    fn the_case_of_the_key_does_not_create_a_second_shortcut() {
        assert_eq!(
            Shortcut::normalize(true, false, false, "F"),
            Shortcut::normalize(true, false, false, "f")
        );
    }

    #[test]
    fn an_assigned_shortcut_wins_over_the_default() {
        let mine = Shortcut::normalize(true, false, true, "p");
        let resolved = resolve(
            &[KeybindingEntry {
                command: CommandId::CommandPalette,
                shortcut: Some(mine.clone()),
            }],
            false,
        );
        let palette = resolved
            .iter()
            .find(|binding| binding.command == CommandId::CommandPalette)
            .unwrap();
        assert_eq!(palette.shortcut.as_ref(), Some(&mine));
        assert!(!palette.is_default);

        // Остальные команды при этом остаются на умолчаниях.
        let search = resolved
            .iter()
            .find(|binding| binding.command == CommandId::Search)
            .unwrap();
        assert!(search.is_default);
    }

    #[test]
    fn a_removed_shortcut_stays_removed() {
        // `None` в записи — это «снято», а не «не задано»: иначе снятое
        // сочетание возвращалось бы умолчанием при следующем чтении файла.
        let resolved = resolve(
            &[KeybindingEntry {
                command: CommandId::Search,
                shortcut: None,
            }],
            false,
        );
        let search = resolved
            .iter()
            .find(|binding| binding.command == CommandId::Search)
            .unwrap();
        assert_eq!(search.shortcut, None);
        assert_eq!(search.display, "");
        assert!(!search.is_default);
    }

    #[test]
    fn a_collision_is_reported_on_both_commands() {
        // Конфликт не запрещается, а показывается — и обеим сторонам, иначе
        // пользователь увидел бы проблему только у той команды, которую правил
        // последней.
        let same = Shortcut::normalize(true, false, false, "k");
        let resolved = resolve(
            &[KeybindingEntry {
                command: CommandId::Search,
                shortcut: Some(same),
            }],
            false,
        );
        let search = resolved
            .iter()
            .find(|binding| binding.command == CommandId::Search)
            .unwrap();
        let palette = resolved
            .iter()
            .find(|binding| binding.command == CommandId::CommandPalette)
            .unwrap();
        assert_eq!(search.conflicts_with, vec![CommandId::CommandPalette]);
        assert_eq!(palette.conflicts_with, vec![CommandId::Search]);
    }

    #[test]
    fn two_removed_shortcuts_do_not_conflict_with_each_other() {
        // Снятое сочетание — это отсутствие сочетания, а не общее значение
        // «пусто», которое совпало бы у всех снятых команд.
        let resolved = resolve(
            &[
                KeybindingEntry {
                    command: CommandId::Search,
                    shortcut: None,
                },
                KeybindingEntry {
                    command: CommandId::Templates,
                    shortcut: None,
                },
            ],
            false,
        );
        for binding in &resolved {
            assert!(binding.conflicts_with.is_empty(), "{binding:?}");
        }
    }

    #[test]
    fn a_letter_shortcut_carries_the_physical_key_name() {
        // Без этого сочетание переставало бы работать при кириллической
        // раскладке: `event.key` для той же клавиши даёт «л», а не «k».
        assert_eq!(
            Shortcut::normalize(true, false, false, "k").code(),
            Some("KeyK".to_string())
        );
        assert_eq!(
            Shortcut::normalize(true, false, false, "1").code(),
            Some("Digit1".to_string())
        );
        // У клавиш без устойчивого имени его и не выдумываем: раскладка на них
        // не влияет, сверка по `key` там достаточна.
        assert_eq!(Shortcut::normalize(true, false, false, "f1").code(), None);

        let resolved = resolve(&[], false);
        let palette = resolved
            .iter()
            .find(|binding| binding.command == CommandId::CommandPalette)
            .unwrap();
        assert_eq!(palette.code, "KeyK");
    }

    #[test]
    fn the_label_follows_the_platform() {
        let shortcut = Shortcut::normalize(true, true, false, "f");
        assert_eq!(shortcut.display(true), "⇧⌘F");
        assert_eq!(shortcut.display(false), "Ctrl+Shift+F");
    }

    #[test]
    fn an_unknown_command_name_is_not_a_command() {
        assert_eq!(CommandId::parse("command-palette"), Some(CommandId::CommandPalette));
        assert_eq!(CommandId::parse("удалить-всё"), None);
    }
}
