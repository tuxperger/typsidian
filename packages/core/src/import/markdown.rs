//! Перевод одного файла Markdown в Typst (фича 021, research.md §2, §3).
//!
//! Разбор — `pulldown-cmark` (CommonMark, таблицы, сноски, флажки, формулы,
//! вики-ссылки); расширения Obsidian, которых в нём нет (`==выделение==`,
//! `%%комментарий%%`, выноски), добираются здесь. Всё, чему нет соответствия,
//! остаётся видимым исходным текстом и попадает в потери с номером строки.
//!
//! # Заметка собирается всегда (FR-008)
//!
//! Пользовательский текст экранируется по правилам разметки Typst; после
//! перевода результат разбирается синтаксическим анализатором Typst, и если
//! ошибки всё же есть — заметка целиком остаётся исходным текстом с потерей
//! [`LossKind::Syntax`]. Страховка не должна срабатывать: тесты прогоняют по
//! ней все образцы.

use std::collections::HashMap;
use std::ops::Range;

use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd,
};

use super::math::{self, MathMode};
use super::names::{
    extension_of, ConvertContext, ResolvedImage, ResolvedLink, Resolver, IMAGE_EXTENSIONS,
};
use super::obsidian::{self, parse_image_hint, parse_wiki_target};
use super::{Loss, LossKind};

/// Результат перевода одной заметки.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Converted {
    pub typst: String,
    pub losses: Vec<Loss>,
}

/// Переводит заметку; ссылки и картинки решает `ctx` (см. [`ConvertContext`]).
pub fn convert(markdown: &str, ctx: &ConvertContext) -> Converted {
    convert_with(markdown, &mut ctx.resolver())
}

/// Тот же перевод с произвольным [`Resolver`]: так им пользуется пакет.
pub fn convert_with(markdown: &str, resolver: &mut dyn Resolver) -> Converted {
    let source = markdown.strip_prefix('\u{feff}').unwrap_or(markdown);
    let source = source.replace("\r\n", "\n");
    let split = obsidian::split_frontmatter(&source);

    let mut losses: Vec<Loss> = Vec::new();
    let mut head = String::new();
    if let Some(yaml) = split.yaml {
        match obsidian::parse_yaml(yaml).and_then(|entries| obsidian::properties_record(&entries)) {
            Some(record) => head = record,
            None if yaml.trim().is_empty() => {}
            None => {
                head = obsidian::properties_comment(yaml);
                losses.push(Loss {
                    kind: LossKind::Properties,
                    line: 1,
                });
            }
        }
    }

    let masked = mask_comments(split.body);
    let line_map = LineMap::new(&masked, split.lines);

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_WIKILINKS);
    let events: Vec<(Event, Range<usize>)> = Parser::new_ext(&masked.text, options)
        .into_offset_iter()
        .collect();

    let mut writer = Writer::new(&events, resolver, &line_map, &masked.comments);
    writer.run();
    let body = writer.finish();
    losses.extend(writer.losses);

    let mut typst = String::new();
    if !head.is_empty() {
        typst.push_str(&head);
        typst.push_str("\n\n");
    }
    typst.push_str(body.trim_end());
    typst.push('\n');

    // Страховка FR-008: результат обязан разбираться без ошибок.
    if typst::syntax::parse(&typst).diagnosis().errors {
        losses.retain(|loss| loss.kind != LossKind::Syntax);
        losses.push(Loss {
            kind: LossKind::Syntax,
            line: 1,
        });
        let mut fallback = String::new();
        if !head.is_empty() && !typst::syntax::parse(&head).diagnosis().errors {
            fallback.push_str(&head);
            fallback.push_str("\n\n");
        }
        fallback.push_str(&raw_block(split.body, ""));
        fallback.push('\n');
        typst = fallback;
    }
    Converted { typst, losses }
}

// ── Комментарии %%…%% ────────────────────────────────────────────────────────

const MARK_OPEN: char = '\u{E000}';
const MARK_CLOSE: char = '\u{E001}';
const HIGHLIGHT: &str = "==";

/// Текст с вынесенными комментариями.
struct Masked {
    text: String,
    comments: Vec<String>,
    /// (позиция метки в `text`, сколько строк занимал комментарий).
    extra_lines: Vec<(usize, usize)>,
}

/// Заменяет `%%…%%` (вне кодовых блоков) метками: комментарий может занимать
/// несколько строк и содержать любую разметку, которую разбор Markdown не
/// должен видеть.
fn mask_comments(source: &str) -> Masked {
    let mut masked = Masked {
        text: String::with_capacity(source.len()),
        comments: Vec::new(),
        extra_lines: Vec::new(),
    };
    let mut fence: Option<(char, usize)> = None;
    let mut open_comment: Option<String> = None;
    for line in source.split_inclusive('\n') {
        if open_comment.is_none() {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            if indent < 4 {
                let marker = trimmed.chars().next();
                if let Some(marker @ ('`' | '~')) = marker {
                    let run = trimmed.chars().take_while(|ch| *ch == marker).count();
                    match fence {
                        None if run >= 3 => fence = Some((marker, run)),
                        Some((open, length))
                            if open == marker
                                && run >= length
                                && trimmed[run..].trim().is_empty() =>
                        {
                            fence = None
                        }
                        _ => {}
                    }
                    masked.text.push_str(line);
                    continue;
                }
            }
            if fence.is_some() {
                masked.text.push_str(line);
                continue;
            }
        }
        let mut rest = line;
        loop {
            match open_comment.take() {
                Some(mut comment) => match rest.find("%%") {
                    Some(end) => {
                        comment.push_str(&rest[..end]);
                        push_comment(&mut masked, comment);
                        rest = &rest[end + 2..];
                    }
                    None => {
                        comment.push_str(rest);
                        open_comment = Some(comment);
                        break;
                    }
                },
                None => match rest.find("%%") {
                    Some(start) => {
                        masked.text.push_str(&rest[..start]);
                        open_comment = Some(String::new());
                        rest = &rest[start + 2..];
                    }
                    None => {
                        masked.text.push_str(rest);
                        break;
                    }
                },
            }
        }
    }
    // Незакрытый `%%` комментарием не считается: возвращаем как был.
    if let Some(comment) = open_comment {
        masked.text.push_str("%%");
        masked.text.push_str(&comment);
    }
    masked
}

fn push_comment(masked: &mut Masked, comment: String) {
    let lines = comment.matches('\n').count();
    let index = masked.comments.len();
    masked.extra_lines.push((masked.text.len(), lines));
    masked.comments.push(comment);
    masked.text.push(MARK_OPEN);
    masked.text.push_str(&index.to_string());
    masked.text.push(MARK_CLOSE);
}

/// Номер строки исходного файла по смещению в тексте после маскировки.
struct LineMap {
    starts: Vec<usize>,
    extra: Vec<(usize, usize)>,
    offset: usize,
}

impl LineMap {
    fn new(masked: &Masked, offset: usize) -> Self {
        let mut starts = vec![0];
        for (index, byte) in masked.text.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(index + 1);
            }
        }
        LineMap {
            starts,
            extra: masked.extra_lines.clone(),
            offset,
        }
    }

    fn line(&self, position: usize) -> u32 {
        let base = self.starts.partition_point(|start| *start <= position);
        let extra: usize = self
            .extra
            .iter()
            .filter(|(at, _)| *at < position)
            .map(|(_, lines)| lines)
            .sum();
        (base + extra + self.offset) as u32
    }
}

// ── Экранирование ────────────────────────────────────────────────────────────

/// Знаки, которые разметка Typst читает как синтаксис в любом месте строки.
const ALWAYS_ESCAPED: &[char] = &['\\', '#', '$', '*', '_', '`', '@', '<', '[', ']', '~'];

/// Пользовательский текст как разметка Typst, которая покажет его буквально.
///
/// `line_start` — перед текстом в строке нет ничего, кроме отступа: тогда
/// `=`, `-`, `+`, `/` и «число с точкой» были бы маркерами заголовка и списков.
pub fn escape_markup(text: &str, line_start: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut at_start = line_start;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();
        if ch.is_ascii_digit() && at_start {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_digit() {
                out.push(chars[j]);
                j += 1;
            }
            if chars.get(j) == Some(&'.') {
                out.push_str("\\.");
                j += 1;
            }
            i = j;
            at_start = false;
            continue;
        }
        let escape = ALWAYS_ESCAPED.contains(&ch)
            || (at_start && matches!(ch, '=' | '-' | '+' | '/'))
            || (ch == '-' && matches!(next, Some('-' | '?')))
            || (ch == '.' && next == Some('.') && chars.get(i + 2) == Some(&'.'))
            || (ch == '/' && matches!(next, Some('/' | '*')) && !ends_with_scheme(&out))
            || (ch == '*' && next == Some('/'));
        if ch == '\n' || ch == '\r' {
            out.push(' ');
        } else if escape {
            out.push('\\');
            out.push(ch);
        } else {
            out.push(ch);
        }
        if !ch.is_whitespace() {
            at_start = false;
        }
        i += 1;
    }
    out
}

/// Строка Typst в кавычках.
pub fn string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => {}
            ch if ch.is_control() => out.push(' '),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Кодовый блок с ограждением длиннее любой серии обратных кавычек внутри.
fn raw_block(content: &str, lang: &str) -> String {
    let longest = content
        .split(|ch| ch != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat((longest + 1).max(3));
    let content = content.trim_end_matches('\n');
    format!("{fence}{lang}\n{content}\n{fence}")
}

fn safe_lang(info: &str) -> String {
    let lang = info.split_whitespace().next().unwrap_or("");
    if lang
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '+' | '-' | '.' | '#'))
    {
        lang.to_owned()
    } else {
        String::new()
    }
}

const DIAGRAMS: &[&str] = &[
    "mermaid",
    "plantuml",
    "graphviz",
    "dot",
    "flowchart",
    "sequence",
    "gantt",
];
const PLUGINS: &[&str] = &[
    "dataview",
    "dataviewjs",
    "tasks",
    "query",
    "button",
    "templater",
    "chart",
    "leaflet",
    "kanban",
    "meta-bind",
    "excalidraw",
];

fn has_scheme(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|ch| ch.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '.' | '-'))
        && scheme.len() > 1
}

/// `%20` → пробел и прочие проценты в локальных путях.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |byte: u8| (byte as char).to_digit(16);
            if let (Some(high), Some(low)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((high * 16 + low) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

// ── Писатель ─────────────────────────────────────────────────────────────────

type Events<'a> = [(Event<'a>, Range<usize>)];

struct Writer<'a, 'r> {
    events: &'a Events<'a>,
    pos: usize,
    out: String,
    line_start: bool,
    indent: String,
    after_call: bool,
    need_blank: bool,
    losses: Vec<Loss>,
    resolver: &'r mut dyn Resolver,
    lines: &'a LineMap,
    comments: &'a [String],
    /// Определения сносок: метка → диапазон событий (без Start и End).
    footnotes: HashMap<String, Range<usize>>,
    /// Осталось парных `==` в текущей строке разметки и открыто ли выделение.
    highlight_left: usize,
    highlight_open: bool,
    /// Глубина вложенности сносок: сноска внутри сноски не раскрывается.
    footnote_depth: usize,
    list_stack: Vec<ListState>,
}

struct ListState {
    /// Номер первого пункта у нумерованного списка.
    start: Option<u64>,
    items_done: usize,
}

impl<'a, 'r> Writer<'a, 'r> {
    fn new(
        events: &'a Events<'a>,
        resolver: &'r mut dyn Resolver,
        lines: &'a LineMap,
        comments: &'a [String],
    ) -> Self {
        let mut footnotes = HashMap::new();
        let mut index = 0;
        while index < events.len() {
            if let Event::Start(Tag::FootnoteDefinition(label)) = &events[index].0 {
                let start = index + 1;
                let mut end = start;
                while end < events.len()
                    && !matches!(events[end].0, Event::End(TagEnd::FootnoteDefinition))
                {
                    end += 1;
                }
                footnotes.insert(label.to_string(), start..end);
                index = end;
            }
            index += 1;
        }
        Writer {
            events,
            pos: 0,
            out: String::new(),
            line_start: true,
            indent: String::new(),
            after_call: false,
            need_blank: false,
            losses: Vec::new(),
            resolver,
            lines,
            comments,
            footnotes,
            highlight_left: 0,
            highlight_open: false,
            footnote_depth: 0,
            list_stack: Vec::new(),
        }
    }

    fn finish(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    fn run(&mut self) {
        let end = self.events.len();
        self.run_range(end);
    }

    /// Обрабатывает события до `end` (не включая).
    fn run_range(&mut self, end: usize) {
        while self.pos < end {
            self.event();
        }
    }

    fn loss(&mut self, kind: LossKind, offset: usize) {
        let loss = Loss {
            kind,
            line: self.lines.line(offset),
        };
        if self.losses.last() != Some(&loss) {
            self.losses.push(loss);
        }
    }

    // ── Вывод ────────────────────────────────────────────────────────────

    fn nl(&mut self) {
        if !self.line_start {
            self.out.push('\n');
            self.line_start = true;
            self.after_call = false;
        }
    }

    fn blank(&mut self) {
        self.nl();
        if !self.out.is_empty() && !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    fn start_block(&mut self) {
        if self.line_start && self.need_blank {
            self.blank();
        }
        self.need_blank = false;
    }

    fn end_block(&mut self) {
        self.nl();
        self.need_blank = true;
    }

    /// Дописывает разметку как есть (без экранирования), одной строкой.
    fn push(&mut self, markup: &str) {
        if markup.is_empty() {
            return;
        }
        if self.line_start {
            self.out.push_str(&self.indent);
            self.line_start = false;
        }
        if self.after_call && (markup.starts_with('.') || markup.starts_with('(')) {
            // `#f[x].y` и `#f[x](y)` Typst прочёл бы продолжением вызова.
            self.out.push_str("\\u{200b}");
        }
        self.out.push_str(markup);
        self.after_call = false;
    }

    /// Текущая строка вывода без отступа контейнера.
    fn current_line(&self) -> &str {
        let line = self.out.rsplit('\n').next().unwrap_or("");
        if self.line_start {
            ""
        } else {
            line.trim_start()
        }
    }

    /// Дописывает пользовательский текст.
    fn text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let line = self.current_line();
        let mut rest = line;
        // Маркеры пунктов в начале строки не мешают считать начало содержимого.
        loop {
            let stripped = rest
                .strip_prefix("- ")
                .or_else(|| rest.strip_prefix("+ "))
                .or_else(|| {
                    let digits = rest.trim_start_matches(|ch: char| ch.is_ascii_digit());
                    (digits.len() < rest.len())
                        .then(|| digits.strip_prefix(". "))
                        .flatten()
                });
            match stripped {
                Some(next) => rest = next,
                None => break,
            }
        }
        let at_content_start = rest.is_empty();
        let digits_only = !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit());
        let mut escaped = escape_markup(text, at_content_start);
        // «1986» и «.» пришли разными событиями: точка после цифр в начале строки
        // была бы маркером нумерованного списка.
        if digits_only && escaped.starts_with('.') {
            escaped.insert(0, '\\');
        }
        self.push(&escaped);
    }

    /// Вызов функции Typst: после него `.` и `(` требуют защиты.
    fn call(&mut self, markup: &str) {
        self.push(markup);
        self.after_call = true;
    }

    /// Строка внутри блока (кода, таблицы) с отступом контейнера.
    fn push_line(&mut self, line: &str) {
        self.nl();
        if !line.is_empty() {
            self.out.push_str(&self.indent);
        }
        self.out.push_str(line);
        self.out.push('\n');
        self.line_start = true;
        self.after_call = false;
    }

    // ── События ──────────────────────────────────────────────────────────

    fn event(&mut self) {
        let offset = self.events[self.pos].1.start;
        let event = self.events[self.pos].0.clone();
        self.pos += 1;
        match event {
            Event::Start(tag) => self.start(tag, offset),
            Event::End(end) => self.end(end),
            Event::Text(text) => self.text_event(&text, offset),
            Event::Code(code) => self.code_span(&code),
            Event::InlineMath(latex) => self.math(&latex, MathMode::Inline, offset),
            Event::DisplayMath(latex) => self.math(&latex, MathMode::Display, offset),
            Event::Html(html) | Event::InlineHtml(html) => self.html(&html, offset),
            Event::FootnoteReference(label) => self.footnote_reference(&label),
            Event::SoftBreak => {
                self.nl();
            }
            Event::HardBreak => {
                self.push(" \\");
                self.nl();
            }
            Event::Rule => {
                self.start_block();
                self.call("#line(length: 100%)");
                self.end_block();
            }
            Event::TaskListMarker(done) => self.push(if done { "☑ " } else { "☐ " }),
        }
    }

    fn end(&mut self, end: TagEnd) {
        match end {
            TagEnd::Paragraph | TagEnd::Heading(_) => {
                self.close_highlight();
                self.end_block();
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                self.end_block();
            }
            _ => {}
        }
    }

    fn close_highlight(&mut self) {
        if self.highlight_open {
            self.push("]");
            self.after_call = true;
            self.highlight_open = false;
        }
    }

    /// Пропускает события до закрывающего тега текущего уровня.
    fn skip_to_end(&mut self) {
        let mut depth = 1usize;
        while self.pos < self.events.len() {
            match &self.events[self.pos].0 {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        self.pos += 1;
                        return;
                    }
                }
                _ => {}
            }
            self.pos += 1;
        }
    }

    /// Конец контейнера, начатого `Start`, по счёту вложенности.
    fn matching_end(&self, from: usize) -> usize {
        let mut depth = 1usize;
        let mut index = from;
        while index < self.events.len() {
            match &self.events[index].0 {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        return index;
                    }
                }
                _ => {}
            }
            index += 1;
        }
        self.events.len()
    }

    /// Текст внутри контейнера без разметки; пробел на мягких переносах.
    fn plain_to_end(&mut self) -> String {
        let end = self.matching_end(self.pos);
        let mut text = String::new();
        for (event, _) in &self.events[self.pos..end] {
            match event {
                Event::Text(t) | Event::Code(t) | Event::InlineMath(t) | Event::DisplayMath(t) => {
                    text.push_str(t)
                }
                Event::SoftBreak | Event::HardBreak => text.push(' '),
                _ => {}
            }
        }
        self.pos = end + 1;
        text
    }

    fn start(&mut self, tag: Tag, offset: usize) {
        match tag {
            Tag::Paragraph => {
                self.start_block();
                self.prepare_highlight();
            }
            Tag::Heading { level, .. } => {
                self.start_block();
                self.prepare_highlight();
                let marks = "=".repeat(heading_depth(level));
                self.push(&format!("{marks} "));
            }
            Tag::BlockQuote(_) => self.block_quote(),
            Tag::CodeBlock(kind) => self.code_block(kind, offset),
            Tag::HtmlBlock => self.html_block(offset),
            Tag::List(start) => {
                self.start_block();
                self.nl();
                self.list_stack.push(ListState {
                    start,
                    items_done: 0,
                });
            }
            Tag::Item => self.item(),
            Tag::FootnoteDefinition(_) => self.skip_to_end(),
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {}
            Tag::Table(alignments) => self.table(&alignments),
            Tag::TableHead | Tag::TableRow | Tag::TableCell => {}
            Tag::Emphasis => self.emphasis(false),
            Tag::Strong => self.emphasis(true),
            Tag::Strikethrough => self.wrapped("#strike["),
            Tag::Superscript => self.wrapped("#super["),
            Tag::Subscript => self.wrapped("#sub["),
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => self.link(link_type, &dest_url, offset),
            Tag::Image {
                link_type,
                dest_url,
                ..
            } => self.image(link_type, &dest_url, offset),
            Tag::MetadataBlock(_) => self.skip_to_end(),
        }
    }

    // ── Выделение ==…== ──────────────────────────────────────────────────

    /// Считает пары `==` в текущем абзаце: нечётный остаток остаётся текстом.
    fn prepare_highlight(&mut self) {
        let end = self.matching_end(self.pos);
        let mut count = 0;
        for (event, _) in &self.events[self.pos..end] {
            if let Event::Text(text) = event {
                count += text.matches(HIGHLIGHT).count();
            }
        }
        self.highlight_left = count - count % 2;
        self.highlight_open = false;
    }

    // ── Текст ────────────────────────────────────────────────────────────

    fn text_event(&mut self, text: &str, offset: usize) {
        // Метки комментариев и парные `==` режут текст на куски.
        let mut rest = text;
        while !rest.is_empty() {
            let comment = rest.find(MARK_OPEN);
            let highlight = rest.find(HIGHLIGHT).filter(|_| self.highlight_left > 0);
            let next = match (comment, highlight) {
                (Some(c), Some(h)) => Some(c.min(h)),
                (c, h) => c.or(h),
            };
            let Some(at) = next else {
                self.text(rest);
                return;
            };
            self.text(&rest[..at]);
            rest = &rest[at..];
            if rest.starts_with(MARK_OPEN) {
                let close = rest.find(MARK_CLOSE).unwrap_or(rest.len() - 1);
                let index: usize = rest[MARK_OPEN.len_utf8()..close]
                    .parse()
                    .unwrap_or(usize::MAX);
                if let Some(comment) = self.comments.get(index) {
                    let safe = comment.replace("*/", "* /").replace("/*", "/ *");
                    // Многострочный комментарий в одну строку: сохраняется
                    // читаемым, а отступы контейнера не ломаются.
                    let flat = safe.replace('\n', " ");
                    self.push(&format!("/* {} */", flat.trim()));
                }
                rest = &rest[(close + MARK_CLOSE.len_utf8()).min(rest.len())..];
            } else {
                // `==`
                rest = &rest[HIGHLIGHT.len()..];
                self.highlight_left -= 1;
                if self.highlight_open {
                    self.push("]");
                    self.after_call = true;
                    self.highlight_open = false;
                } else {
                    self.push("#highlight[");
                    self.highlight_open = true;
                }
            }
        }
        let _ = offset;
    }

    fn code_span(&mut self, code: &str) {
        if !code.is_empty()
            && !code.contains(['`', '\n', '\r'])
            && !code.starts_with(' ')
            && !code.ends_with(' ')
        {
            self.push(&format!("`{code}`"));
        } else {
            self.call(&format!("#raw({})", string_literal(code)));
        }
    }

    fn math(&mut self, latex: &str, mode: MathMode, offset: usize) {
        match math::convert(latex, mode) {
            Some(typst) => self.push(&typst),
            None => {
                self.loss(LossKind::Math, offset);
                let source = match mode {
                    MathMode::Inline => format!("${latex}$"),
                    MathMode::Display => format!("$${latex}$$"),
                };
                self.call(&format!("#raw({})", string_literal(&source)));
            }
        }
    }

    /// Строчный HTML: `<br>` — перенос строки, комментарий — комментарий
    /// Typst, остальное — исходным текстом.
    fn html(&mut self, html: &str, offset: usize) {
        let trimmed = html.trim();
        if matches!(trimmed.to_lowercase().as_str(), "<br>" | "<br/>" | "<br />") {
            self.push(" \\");
            self.nl();
            return;
        }
        if let Some(comment) = html_comment(trimmed) {
            self.push(&comment);
            return;
        }
        self.loss(LossKind::Html, offset);
        self.call(&format!("#raw({})", string_literal(html)));
    }

    /// Блок HTML целиком: комментарий или `#raw(block: true, …)`.
    fn html_block(&mut self, offset: usize) {
        let end = self.matching_end(self.pos);
        let mut block = String::new();
        for (event, _) in &self.events[self.pos..end] {
            if let Event::Html(html) = event {
                block.push_str(html);
            }
        }
        self.pos = end + 1;
        let trimmed = block.trim();
        self.start_block();
        if let Some(comment) = html_comment(trimmed) {
            self.push(&comment);
        } else {
            self.loss(LossKind::Html, offset);
            self.call(&format!("#raw(block: true, {})", string_literal(trimmed)));
        }
        self.end_block();
    }

    fn footnote_reference(&mut self, label: &str) {
        let range = self.footnotes.get(label).cloned();
        let Some(range) = range.filter(|_| self.footnote_depth == 0) else {
            self.text(&format!("[^{label}]"));
            return;
        };
        let mut body = String::new();
        let mut inner_losses = Vec::new();
        {
            let events = self.events;
            let mut sub = Writer::new(events, &mut *self.resolver, self.lines, self.comments);
            sub.footnote_depth = self.footnote_depth + 1;
            sub.pos = range.start;
            sub.run_range(range.end);
            body.push_str(sub.finish().trim());
            inner_losses.append(&mut sub.losses);
        }
        self.losses.extend(inner_losses);
        // Абзацы сноски в одну строку: содержимое `[…]` остаётся однострочным.
        let mut flat = body.replace("\n\n", " \\ ").replace('\n', " ");
        if flat.starts_with("[[") {
            flat.insert(0, ' ');
        }
        self.call(&format!("#footnote[{flat}]"));
    }

    // ── Блоки ────────────────────────────────────────────────────────────

    fn block_quote(&mut self) {
        self.start_block();
        self.push("#quote(block: true)[");
        self.nl();
        let saved = self.indent.len();
        self.indent.push_str("  ");
        self.need_blank = false;

        // Выноска Obsidian: первая строка `[!тип] Заголовок`.
        if let Some((title, skip_to)) = self.callout_title() {
            self.pos = skip_to;
            self.start_block();
            self.push(&format!("*{}*", escape_markup(&title, false)));
            self.end_block();
            // Остаток абзаца после заголовка — новый абзац.
            if matches!(self.events.get(self.pos), Some((Event::SoftBreak, _))) {
                self.pos += 1;
            }
            if !matches!(
                self.events.get(self.pos),
                Some((Event::End(TagEnd::Paragraph), _))
            ) {
                self.start_block();
                self.prepare_highlight();
            }
        }

        let end = self.matching_end(self.pos);
        self.run_range(end);
        self.pos = end + 1;
        self.indent.truncate(saved);
        self.nl();
        self.push("]");
        self.end_block();
    }

    /// Если цитата — выноска, возвращает заголовок и позицию после него.
    fn callout_title(&self) -> Option<(String, usize)> {
        let mut index = self.pos;
        if !matches!(self.events.get(index)?.0, Event::Start(Tag::Paragraph)) {
            return None;
        }
        index += 1;
        let mut line = String::new();
        let mut end = index;
        while let Some((event, _)) = self.events.get(end) {
            match event {
                Event::Text(text) => line.push_str(text),
                _ => break,
            }
            end += 1;
        }
        let (kind, title) = obsidian::parse_callout(&line)?;
        let title = if title.is_empty() {
            obsidian::callout_default_title(&kind)
        } else {
            title
        };
        Some((title, end))
    }

    fn code_block(&mut self, kind: CodeBlockKind, offset: usize) {
        self.start_block();
        let lang = match &kind {
            CodeBlockKind::Fenced(info) => safe_lang(info),
            CodeBlockKind::Indented => String::new(),
        };
        let mut content = String::new();
        while let Some((event, _)) = self.events.get(self.pos) {
            match event {
                Event::Text(text) => content.push_str(text),
                Event::End(_) => {
                    self.pos += 1;
                    break;
                }
                _ => {}
            }
            self.pos += 1;
        }
        let lower = lang.to_lowercase();
        if DIAGRAMS.contains(&lower.as_str()) {
            self.loss(LossKind::Diagram, offset);
        } else if PLUGINS.contains(&lower.as_str())
            || lower.starts_with("ad-")
            || lower.starts_with("meta-bind")
        {
            self.loss(LossKind::PluginBlock, offset);
        }
        let block = raw_block(&content, &lang);
        for line in block.split('\n') {
            self.push_line(line);
        }
        self.end_block();
    }

    fn item(&mut self) {
        let (marker, width) = match self.list_stack.last_mut() {
            Some(state) => {
                let first = state.items_done == 0;
                state.items_done += 1;
                match state.start {
                    None => ("- ".to_owned(), 2),
                    Some(start) if first && start != 1 => {
                        let marker = format!("{start}. ");
                        let width = marker.len();
                        (marker, width)
                    }
                    Some(_) => ("+ ".to_owned(), 2),
                }
            }
            None => ("- ".to_owned(), 2),
        };
        self.nl();
        self.push(&marker);
        let saved = self.indent.len();
        self.indent.push_str(&" ".repeat(width));
        self.need_blank = false;
        let end = self.matching_end(self.pos);
        self.prepare_highlight_inline(end);
        self.run_range(end);
        self.pos = end + 1;
        self.close_highlight();
        self.indent.truncate(saved);
        self.nl();
        self.need_blank = false;
    }

    /// В «плотном» пункте текст лежит прямо в Item, без Paragraph.
    fn prepare_highlight_inline(&mut self, end: usize) {
        let mut count = 0;
        let mut depth = 0usize;
        for (event, _) in &self.events[self.pos..end] {
            match event {
                Event::Start(Tag::List(_)) => depth += 1,
                Event::End(TagEnd::List(_)) => depth = depth.saturating_sub(1),
                Event::Text(text) if depth == 0 => count += text.matches(HIGHLIGHT).count(),
                _ => {}
            }
        }
        self.highlight_left = count - count % 2;
        self.highlight_open = false;
    }

    fn table(&mut self, alignments: &[Alignment]) {
        self.start_block();
        let columns = alignments.len().max(1);
        let mut header: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<String>> = Vec::new();
        let end = self.matching_end(self.pos);
        while self.pos < end {
            match self.events[self.pos].0.clone() {
                Event::Start(Tag::TableHead) => {
                    self.pos += 1;
                    header = self.cells_until(TagEnd::TableHead, columns);
                }
                Event::Start(Tag::TableRow) => {
                    self.pos += 1;
                    rows.push(self.cells_until(TagEnd::TableRow, columns));
                }
                _ => self.pos += 1,
            }
        }
        self.pos = end + 1;

        let mut lines = vec!["#table(".to_owned(), format!("  columns: {columns},")];
        if alignments.iter().any(|a| *a != Alignment::None) {
            let names: Vec<&str> = (0..columns)
                .map(|i| match alignments.get(i) {
                    Some(Alignment::Left) => "left",
                    Some(Alignment::Center) => "center",
                    Some(Alignment::Right) => "right",
                    _ => "auto",
                })
                .collect();
            lines.push(format!("  align: ({}),", names.join(", ")));
        }
        let cell = |text: &String| format!("[{text}]");
        lines.push(format!(
            "  table.header({}),",
            header.iter().map(cell).collect::<Vec<_>>().join(", ")
        ));
        for row in &rows {
            lines.push(format!(
                "  {},",
                row.iter().map(cell).collect::<Vec<_>>().join(", ")
            ));
        }
        lines.push(")".to_owned());
        for line in lines {
            self.push_line(&line);
        }
        self.after_call = false;
        self.end_block();
    }

    /// Ячейки строки: каждая — содержимое `[…]` в одну строку.
    fn cells_until(&mut self, stop: TagEnd, columns: usize) -> Vec<String> {
        let mut cells: Vec<String> = Vec::new();
        while self.pos < self.events.len() {
            match self.events[self.pos].0.clone() {
                Event::End(end) if end == stop => {
                    self.pos += 1;
                    break;
                }
                Event::Start(Tag::TableCell) => {
                    self.pos += 1;
                    let end = self.matching_end(self.pos);
                    cells.push(self.capture_cell(end));
                    self.pos = end + 1;
                }
                _ => self.pos += 1,
            }
        }
        cells.resize(columns, String::new());
        cells
    }

    fn capture_cell(&mut self, end: usize) -> String {
        let saved_out = std::mem::take(&mut self.out);
        let saved_indent = std::mem::take(&mut self.indent);
        let saved_start = self.line_start;
        let saved_call = self.after_call;
        self.line_start = true;
        self.after_call = false;
        self.prepare_highlight_inline(end);
        self.run_range(end);
        self.close_highlight();
        let cell = std::mem::replace(&mut self.out, saved_out);
        self.indent = saved_indent;
        self.line_start = saved_start;
        self.after_call = saved_call;
        let cell = cell.replace('\n', " ").trim().to_owned();
        // `[` ячейки и `[[` ссылки слились бы в `[[[`.
        if cell.starts_with("[[") {
            format!(" {cell}")
        } else {
            cell
        }
    }

    // ── Выделение, ссылки, картинки ──────────────────────────────────────

    /// Содержимое внутри `#функция[ … ]` до закрывающего тега.
    fn wrapped(&mut self, open: &str) {
        self.push(open);
        let end = self.matching_end(self.pos);
        self.run_range(end);
        self.pos = end + 1;
        self.push("]");
        self.after_call = true;
    }

    fn emphasis(&mut self, strong: bool) {
        let end = self.matching_end(self.pos);
        let previous_alnum = self
            .out
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_alphanumeric())
            && !self.line_start;
        let next_alnum = matches!(
            self.events.get(end + 1),
            Some((Event::Text(text), _)) if text.chars().next().is_some_and(char::is_alphanumeric)
        );
        let first_blank = matches!(
            self.events.get(self.pos),
            Some((Event::Text(text), _)) if text.starts_with(char::is_whitespace)
        );
        let last_blank = end > 0
            && matches!(
                self.events.get(end - 1),
                Some((Event::Text(text), _)) if text.ends_with(char::is_whitespace)
            );
        let empty = end == self.pos;
        let marker_ok = !previous_alnum
            && !next_alnum
            && !first_blank
            && !last_blank
            && !empty
            && !self.after_call;
        if marker_ok {
            let mark = if strong { "*" } else { "_" };
            self.push(mark);
            self.run_range(end);
            self.pos = end + 1;
            self.push(mark);
        } else {
            self.wrapped(if strong { "#strong[" } else { "#emph[" });
        }
    }

    fn link(&mut self, link_type: LinkType, dest: &str, offset: usize) {
        if let LinkType::WikiLink { has_pothole } = link_type {
            return self.wiki_link(dest, has_pothole, offset);
        }
        let url = match link_type {
            LinkType::Email => format!("mailto:{dest}"),
            _ => dest.to_owned(),
        };
        let (path_part, anchor) = match url.split_once('#') {
            Some((path, anchor)) => (path.to_owned(), Some(anchor.to_owned())),
            None => (url.clone(), None),
        };
        // Якорь внутри той же заметки: остаётся только текст.
        if url.starts_with('#') {
            let end = self.matching_end(self.pos);
            self.run_range(end);
            self.pos = end + 1;
            return;
        }
        if has_scheme(&url) {
            let open = format!("#link({})[", string_literal(&url));
            return self.wrapped(&open);
        }
        let decoded = percent_decode(&path_part);
        let ext = extension_of(&decoded);
        if ext == "md" || ext == "markdown" {
            let label = self.plain_to_end();
            return self.note_link(&decoded, anchor.as_deref(), Some(label), offset);
        }
        let open = format!("#link({})[", string_literal(&percent_decode(&url)));
        self.wrapped(&open);
    }

    fn wiki_link(&mut self, dest: &str, has_pothole: bool, offset: usize) {
        let label = self.plain_to_end();
        let target = parse_wiki_target(dest);
        let label = has_pothole.then_some(label);
        if target.path.is_empty() {
            // Ссылка на раздел этой же заметки.
            self.loss(
                if target.block.is_some() {
                    LossKind::BlockLink
                } else {
                    LossKind::SectionLink
                },
                offset,
            );
            let shown = label
                .or(target.heading)
                .or(target.block)
                .unwrap_or_default();
            self.text(&shown);
            return;
        }
        let anchor = match (&target.heading, &target.block) {
            (Some(heading), _) => Some(heading.clone()),
            (None, Some(block)) => Some(format!("^{block}")),
            _ => None,
        };
        self.note_link(&target.path, anchor.as_deref(), label, offset);
    }

    /// Ссылка vault на заметку (FR-005).
    fn note_link(
        &mut self,
        path: &str,
        anchor: Option<&str>,
        label: Option<String>,
        offset: usize,
    ) {
        let shown_name = super::names::stem_of(super::names::file_name_of(
            path.trim_end_matches(".md").trim_end_matches(".markdown"),
        ))
        .to_owned();
        let shown_name = if shown_name.is_empty() {
            path.to_owned()
        } else {
            shown_name
        };
        if let Some(anchor) = anchor {
            self.loss(
                if anchor.starts_with('^') {
                    LossKind::BlockLink
                } else {
                    LossKind::SectionLink
                },
                offset,
            );
        }
        // Без своего текста ссылка на раздел показывает «Имя > Раздел», как Obsidian.
        let display = label.filter(|l| !l.trim().is_empty()).or_else(|| {
            anchor
                .filter(|a| !a.starts_with('^'))
                .map(|a| format!("{shown_name} > {a}"))
        });
        match self.resolver.link(path) {
            ResolvedLink::Note { inner, ambiguous } => {
                if ambiguous {
                    self.loss(LossKind::AmbiguousTarget, offset);
                }
                let display = display.or_else(|| (inner != shown_name).then(|| shown_name.clone()));
                self.write_vault_link(&inner, display.as_deref());
            }
            ResolvedLink::File => {
                self.text(display.as_deref().unwrap_or(&shown_name));
            }
            ResolvedLink::Missing => {
                self.loss(LossKind::MissingTarget, offset);
                let written = path.trim_end_matches(".md").trim_end_matches(".markdown");
                if super::names::is_link_safe(written) && !written.is_empty() {
                    self.write_vault_link(written, display.as_deref());
                } else {
                    self.text(display.as_deref().unwrap_or(&shown_name));
                }
            }
        }
    }

    fn write_vault_link(&mut self, inner: &str, label: Option<&str>) {
        // `[[[` читается как ссылка с целью, начинающейся на `[`.
        if !self.line_start && self.out.ends_with('[') {
            self.push(" ");
        }
        let record = match label.map(|l| l.replace(['\n', '\r'], " ")) {
            Some(label) if !label.trim().is_empty() => {
                let cleaned: String = label.replace(['[', ']', '|'], "");
                format!("[[{inner}|{}]]", escape_markup(cleaned.trim(), false))
            }
            _ => format!("[[{inner}]]"),
        };
        self.push(&record);
    }

    fn image(&mut self, link_type: LinkType, dest: &str, offset: usize) {
        let has_pothole = matches!(link_type, LinkType::WikiLink { has_pothole: true });
        let is_wiki = matches!(link_type, LinkType::WikiLink { .. });
        let alt_text = self.plain_to_end();
        let hint = if is_wiki && !has_pothole {
            obsidian::ImageHint::default()
        } else {
            parse_image_hint(&alt_text)
        };
        let target = parse_wiki_target(dest);
        let url = if is_wiki {
            target.path.clone()
        } else {
            dest.to_owned()
        };

        if !is_wiki && has_scheme(&url) {
            // Сети нет: картинка по адресу остаётся ссылкой.
            let shown = if hint.alt.is_empty() {
                url.clone()
            } else {
                hint.alt.clone()
            };
            self.call(&format!(
                "#link({})[{}]",
                string_literal(&url),
                escape_markup(&shown, false)
            ));
            return;
        }
        let path = percent_decode(url.split('#').next().unwrap_or(&url));
        let ext = extension_of(&path);

        if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            match self.resolver.image(&path) {
                ResolvedImage::Image { path: markup } => {
                    let mut args = vec![string_literal(&markup)];
                    if let Some(width) = hint.width {
                        args.push(format!("width: {width}pt"));
                    }
                    if let Some(height) = hint.height {
                        args.push(format!("height: {height}pt"));
                    }
                    self.call(&format!("#image({})", args.join(", ")));
                }
                ResolvedImage::Missing => {
                    self.loss(LossKind::MissingTarget, offset);
                    self.call(&format!(
                        "#raw({})",
                        string_literal(&image_source(is_wiki, dest, &alt_text))
                    ));
                }
            }
            return;
        }

        if is_wiki && (ext.is_empty() || ext == "md" || ext == "markdown") {
            // Встраивание заметки заменено ссылкой.
            self.loss(LossKind::Embed, offset);
            let anchor = target
                .heading
                .clone()
                .or(target.block.as_ref().map(|b| format!("^{b}")));
            let label = has_pothole.then_some(alt_text);
            return self.note_link(&target.path, anchor.as_deref(), label, offset);
        }

        // Файл не-картинка (pdf, аудио, неподдерживаемый формат).
        self.loss(LossKind::Embed, offset);
        self.call(&format!(
            "#raw({})",
            string_literal(&image_source(is_wiki, dest, &alt_text))
        ));
    }
}

fn image_source(is_wiki: bool, dest: &str, alt: &str) -> String {
    if is_wiki {
        format!("![[{dest}]]")
    } else {
        format!("![{alt}]({dest})")
    }
}

fn heading_depth(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// `<!-- … -->` целиком как комментарий Typst.
fn html_comment(html: &str) -> Option<String> {
    let inner = html.strip_prefix("<!--")?.strip_suffix("-->")?;
    let safe = inner
        .replace("*/", "* /")
        .replace("/*", "/ *")
        .replace('\n', " ");
    Some(format!("/* {} */", safe.trim()))
}

/// Разбирается ли текст Typst с ошибками — для тестов и страховки.
pub fn has_syntax_errors(typst: &str) -> bool {
    typst::syntax::parse(typst).diagnosis().errors
}

/// Текст перед `//` — схема адреса (`http:`, `https:`): так `//` не
/// комментарий, а начало адреса, и экранировать его не нужно.
fn ends_with_scheme(out: &str) -> bool {
    let tail: String = out
        .chars()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>()
        .to_ascii_lowercase();
    tail.ends_with("http:") || tail.ends_with("https:")
}
