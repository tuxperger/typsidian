//! Генератор машинной части схемы параметров (фича 008, research.md §1).
//!
//! ```text
//! cargo run -p typsidian-core --example element-schema            # дописать схему
//! cargo run -p typsidian-core --example element-schema -- --check # только показать расхождения
//! ```
//!
//! Читает `src/elements/schema.json` и для каждого элемента, у которого нет
//! `same_as`, ставит машинные поля каждого параметра так, как их описывает
//! Typst: вид значения, умолчание, позиционный, обязательный, допускается ли в
//! `set`. Недостающие параметры дописываются, несуществующие — убираются.
//!
//! Подписи и признак «основной» не трогаются: их пишет человек. У нового
//! параметра подпись берётся из словаря частых имён ниже, а если её там нет —
//! остаётся пустой, и тест схемы напомнит о ней.
//!
//! У элементов-разметки (`markup-*`) набор параметров — профиль разметки,
//! выбранный руками (research.md §7): генератор только уточняет машинные поля
//! уже перечисленных.

use std::path::PathBuf;

use serde_json::{json, Map, Value};
use typsidian_core::elements::schema::{machine_params, schema_problems, ElementSyntax, MachineParam};

fn main() {
    let check = std::env::args().any(|arg| arg == "--check");
    if check {
        let problems = schema_problems();
        if problems.is_empty() {
            println!("схема совпадает с Typst");
        } else {
            println!("{}", problems.join("\n"));
            std::process::exit(1);
        }
        return;
    }

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/elements/schema.json");
    let text = std::fs::read_to_string(&path).expect("чтение schema.json");
    let mut root: Value = serde_json::from_str(&text).expect("schema.json разбирается");

    let elements = root["elements"]
        .as_array_mut()
        .expect("elements — массив");
    for element in elements {
        if element.get("same_as").is_some() {
            continue;
        }
        let id = element["id"].as_str().unwrap_or_default().to_string();
        let typst = element["typst"].as_str().unwrap_or_default().to_string();
        let syntax: ElementSyntax =
            serde_json::from_value(element["syntax"].clone()).expect("syntax");
        let Some(machine) = machine_params(&typst, syntax) else {
            eprintln!("{id}: в Typst нет функции {typst} — пропущен");
            continue;
        };
        let markup = matches!(
            syntax,
            ElementSyntax::MarkupHeading | ElementSyntax::MarkupRaw | ElementSyntax::MarkupEquation
        );

        let existing: Vec<Value> = element["params"].as_array().cloned().unwrap_or_default();
        let find = |name: &str| {
            existing
                .iter()
                .find(|param| param["name"].as_str() == Some(name))
                .cloned()
        };

        let wanted: Vec<&MachineParam> = if markup {
            existing
                .iter()
                .filter_map(|param| param["name"].as_str())
                .filter_map(|name| machine.iter().find(|m| m.name == name))
                .collect()
        } else {
            machine.iter().collect()
        };

        let params: Vec<Value> = wanted
            .into_iter()
            .map(|m| {
                let old = find(&m.name).unwrap_or_else(|| json!({}));
                let title = old["title"]
                    .as_str()
                    .filter(|title| !title.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| dictionary(&m.name).unwrap_or_default().to_string());
                let mut param = Map::new();
                param.insert("name".into(), json!(m.name));
                param.insert("title".into(), json!(title));
                param.insert("main".into(), json!(old["main"].as_bool().unwrap_or(false)));
                param.insert("kind".into(), serde_json::to_value(&m.kind).unwrap());
                param.insert("default".into(), json!(m.default));
                param.insert("positional".into(), json!(m.positional));
                param.insert("required".into(), json!(m.required));
                param.insert("settable".into(), json!(m.settable));
                param.insert("variadic".into(), json!(m.variadic));
                for key in ["override_kind", "override_reason"] {
                    if !old[key].is_null() {
                        param.insert(key.into(), old[key].clone());
                    }
                }
                Value::Object(param)
            })
            .collect();
        element["params"] = Value::Array(params);
    }

    let pretty = serde_json::to_string_pretty(&root).expect("сериализация");
    std::fs::write(&path, pretty + "\n").expect("запись schema.json");
    println!("schema.json обновлён: {}", path.display());
}

/// Подписи частых имён параметров. Всё, чего здесь нет, подписывается руками.
fn dictionary(name: &str) -> Option<&'static str> {
    Some(match name {
        "body" => "Содержимое",
        "width" => "Ширина",
        "height" => "Высота",
        "size" => "Размер",
        "fill" => "Заливка",
        "stroke" => "Обводка",
        "radius" => "Скругление углов",
        "inset" => "Внутренний отступ",
        "outset" => "Внешний отступ",
        "clip" => "Обрезать выходящее",
        "breakable" => "Разрывать между страницами",
        "spacing" => "Интервал",
        "above" => "Отступ сверху",
        "below" => "Отступ снизу",
        "sticky" => "Держать со следующим",
        "baseline" => "Базовая линия",
        "align" => "Выравнивание",
        "alignment" => "Выравнивание",
        "columns" => "Колонки",
        "rows" => "Строки",
        "gutter" => "Промежуток",
        "column-gutter" => "Промежуток между колонками",
        "row-gutter" => "Промежуток между строками",
        "children" => "Элементы",
        "caption" => "Подпись",
        "placement" => "Размещение",
        "scope" => "Область размещения",
        "kind" => "Вид",
        "supplement" => "Приставка к номеру",
        "numbering" => "Нумерация",
        "gap" => "Зазор до подписи",
        "outlined" => "В оглавлении",
        "path" => "Файл",
        "source" => "Файл",
        "format" => "Формат",
        "alt" => "Замещающий текст",
        "fit" => "Вписывание",
        "scaling" => "Масштабирование",
        "icc" => "Цветовой профиль",
        "page" => "Страница",
        "title" => "Заголовок",
        "depth" => "Глубина",
        "indent" => "Отступ",
        "level" => "Уровень",
        "lang" => "Язык",
        "region" => "Регион",
        "block" => "Отдельным блоком",
        "dx" => "Сдвиг по горизонтали",
        "dy" => "Сдвиг по вертикали",
        "dir" => "Направление",
        "count" => "Число колонок",
        "angle" => "Угол",
        "origin" => "Точка отсчёта",
        "reflow" => "Перестроить макет",
        "x" => "По горизонтали",
        "y" => "По вертикали",
        "weak" => "Слабый",
        "amount" => "Величина",
        "to" => "До страницы",
        "text" => "Текст",
        "dest" => "Адрес",
        "key" => "Источник",
        "form" => "Форма ссылки",
        "style" => "Стиль",
        "full" => "Все источники",
        "attribution" => "Источник цитаты",
        "quotes" => "Кавычки",
        "marker" => "Маркер",
        "tight" => "Плотный",
        "start" => "Начало нумерации",
        "full-numbering" => "Полная нумерация",
        "number-align" => "Выравнивание номера",
        "body-indent" => "Отступ текста",
        "item-spacing" => "Интервал между пунктами",
        "reversed" => "Обратный порядок",
        "font" => "Шрифт",
        "weight" => "Насыщенность",
        "tracking" => "Трекинг",
        "leading" => "Интерлиньяж",
        "justify" => "Выключка по ширине",
        "first-line-indent" => "Отступ первой строки",
        "hanging-indent" => "Висячий отступ",
        "linebreaks" => "Переносы строк",
        "paper" => "Формат бумаги",
        "margin" => "Поля",
        "flipped" => "Альбомная",
        "binding" => "Сторона переплёта",
        "header" => "Верхний колонтитул",
        "footer" => "Нижний колонтитул",
        "background" => "Фон",
        "foreground" => "Передний план",
        "author" => "Автор",
        "keywords" => "Ключевые слова",
        "date" => "Дата",
        "description" => "Описание",
        "offset" => "Смещение",
        "evade" => "Обходить линии",
        "background-fill" => "Заливка фона",
        "extent" => "Выступ",
        "length" => "Длина",
        "vertices" => "Число вершин",
        "delim" => "Скобки",
        "augment" => "Разделители",
        "num" => "Числитель",
        "denom" => "Знаменатель",
        "index" => "Показатель корня",
        "radicand" => "Подкоренное выражение",
        "upper" => "Верхнее",
        "lower" => "Нижнее",
        "base" => "Основание",
        "accent" => "Надстрочный знак",
        "annotation" => "Подпись",
        "limits" => "Пределы сверху и снизу",
        "cancel" => "Зачеркнуть",
        "inverted" => "Обратный наклон",
        "cross" => "Крест-накрест",
        "rotation" => "Поворот",
        "reverse" => "Обратный порядок",
        "group" => "Группировать с соседними",
        "sources" => "Файл библиографии",
        "target" => "Что собирать",
        "encoding" => "Кодировка",
        "top-edge" => "Верхняя граница строки",
        "bottom-edge" => "Нижняя граница строки",
        "end" => "Конечная точка",
        "marker-align" => "Выравнивание маркера",
        "words" => "Число слов",
        "dotless" => "Без точки над i и j",
        "t" => "Сверху",
        "b" => "Снизу",
        "tl" => "Сверху слева",
        "bl" => "Снизу слева",
        "tr" => "Сверху справа",
        "br" => "Снизу справа",
        "numbers" => "Числа",
        "left" => "Слева",
        "right" => "Справа",
        "top" => "Сверху",
        "bottom" => "Снизу",
        "rest" => "Со всех сторон",
        "bleed" => "Выпуск под обрез",
        "header-ascent" => "Подъём верхнего колонтитула",
        "footer-descent" => "Спуск нижнего колонтитула",
        "clearance" => "Зазор до текста",
        "float" => "Плавающее размещение",
        "factor" => "Масштаб",
        "bookmarked" => "В закладках PDF",
        "justification-limits" => "Пределы растяжения при выключке",
        "alternates" => "Альтернативные глифы",
        "cjk-latin-spacing" => "Пробел между CJK и латиницей",
        "costs" => "Штрафы вёрстки",
        "discretionary-ligatures" => "Дискреционные лигатуры",
        "fallback" => "Подстановка шрифта",
        "features" => "Функции OpenType",
        "fractions" => "Дроби OpenType",
        "historical-ligatures" => "Исторические лигатуры",
        "hyphenate" => "Переносы слов",
        "kerning" => "Кернинг",
        "ligatures" => "Лигатуры",
        "number-type" => "Вид цифр",
        "number-width" => "Ширина цифр",
        "overhang" => "Висячая пунктуация",
        "script" => "Письменность",
        "slashed-zero" => "Перечёркнутый ноль",
        "stretch" => "Ширина начертания",
        "stylistic-set" => "Стилистический набор",
        "variations" => "Вариации шрифта",
        "all" => "Все буквы",
        "typographic" => "Типографские глифы",
        _ => return None,
    })
}
