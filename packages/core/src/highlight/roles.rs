//! Роль узла: `typst::syntax::highlight` и уточнения под палитру (research.md §4).
//!
//! Уточнения проверяются раньше тега Typst: они опираются на вид узла и его
//! соседей, а тег Typst для этих мест либо пуст, либо слишком общий.

use typst::syntax::{highlight, LinkedNode, SyntaxKind, Tag};

use super::{StyleFlags, SyntaxRole};

/// Что узел даёт своему поддереву.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NodeRole {
    /// Собственная роль узла; у контейнера — роль листьев без своей.
    Role(SyntaxRole),
    /// Начертание для всего поддерева: `*…*`, `_…_`, заголовок.
    Style(StyleFlags),
}

pub(super) fn role_of(node: &LinkedNode) -> Option<NodeRole> {
    use NodeRole::{Role, Style};
    let kind = node.kind();
    let parent = node.parent_kind();
    let refined = match kind {
        SyntaxKind::Strong => Some(Style(StyleFlags::STRONG)),
        SyntaxKind::Emph => Some(Style(StyleFlags::EMPH)),
        SyntaxKind::Heading => Some(Style(StyleFlags::HEADING)),
        SyntaxKind::HeadingMarker => Some(Role(SyntaxRole::HeadingMarker)),
        SyntaxKind::Star if parent == Some(SyntaxKind::Strong) => {
            Some(Role(SyntaxRole::MarkupMarker))
        }
        SyntaxKind::Underscore if parent == Some(SyntaxKind::Emph) => {
            Some(Role(SyntaxRole::MarkupMarker))
        }
        SyntaxKind::RawDelim => Some(Role(SyntaxRole::MarkupMarker)),
        SyntaxKind::Bool | SyntaxKind::None | SyntaxKind::Auto => Some(Role(SyntaxRole::Constant)),
        // Решётка перехода в код — всегда ключевая: у Typst она перенимает цвет
        // выражения справа, а в макете отделена от имени функции.
        SyntaxKind::Hash => Some(Role(SyntaxRole::Keyword)),
        SyntaxKind::Ident if is_arg_name(node) => Some(Role(SyntaxRole::ArgName)),
        SyntaxKind::Ident | SyntaxKind::MathIdent | SyntaxKind::Dot if in_callee_path(node) => {
            Some(Role(SyntaxRole::Function))
        }
        SyntaxKind::MathText => Some(Role(SyntaxRole::MathIdent)),
        _ => None,
    };
    if refined.is_some() {
        return refined;
    }
    let role = match highlight(node)? {
        Tag::Comment => SyntaxRole::Comment,
        Tag::Punctuation | Tag::MathGroupingParens => SyntaxRole::Punctuation,
        Tag::Escape => SyntaxRole::Escape,
        // Начертание выделения и заголовка задаёт `Style` выше; сюда эти теги
        // не доходят.
        Tag::Strong | Tag::Emph | Tag::Heading => return None,
        Tag::Link => SyntaxRole::Link,
        Tag::Raw => SyntaxRole::Raw,
        Tag::Label => SyntaxRole::Label,
        Tag::Ref => SyntaxRole::Ref,
        Tag::ListMarker => SyntaxRole::ListMarker,
        Tag::ListTerm => SyntaxRole::ListTerm,
        Tag::MathDelimiter => SyntaxRole::MathDelimiter,
        Tag::MathOperator => SyntaxRole::MathOperator,
        Tag::Keyword => SyntaxRole::Keyword,
        Tag::Operator => SyntaxRole::Operator,
        Tag::Number => SyntaxRole::Number,
        Tag::String => SyntaxRole::String,
        Tag::Function => SyntaxRole::Function,
        Tag::Interpolated if kind == SyntaxKind::MathIdent => SyntaxRole::MathIdent,
        Tag::Interpolated => SyntaxRole::Variable,
        // Ошибки сборки — вне скоупа: текст с ошибкой выглядит как обычный.
        Tag::Error => return None,
    };
    Some(Role(role))
}

/// Имя именованного аргумента: `caption` в `caption: [ … ]`.
fn is_arg_name(node: &LinkedNode) -> bool {
    node.parent_kind() == Some(SyntaxKind::Named) && node.index() == 0
}

/// Звено пути вызываемой функции: `calc`, `.` и `pow` в `calc.pow(…)`.
///
/// Typst красит функцией только последнее звено; путь целиком нужен по
/// AS 2.3 спеки. Узел — звено, если цепочка доступов к полю над ним (каждый —
/// первый ребёнок следующего) и есть то, что вызывается: первый ребёнок вызова.
fn in_callee_path(node: &LinkedNode) -> bool {
    let access = |kind| matches!(kind, SyntaxKind::FieldAccess | SyntaxKind::MathFieldAccess);
    let Some(mut ancestor) = node.parent() else {
        return false;
    };
    if !access(ancestor.kind()) {
        return false;
    }
    loop {
        let Some(parent) = ancestor.parent() else {
            return false;
        };
        if access(parent.kind()) && ancestor.index() == 0 {
            ancestor = parent;
            continue;
        }
        return matches!(parent.kind(), SyntaxKind::FuncCall | SyntaxKind::MathCall)
            && ancestor.index() == 0;
    }
}
