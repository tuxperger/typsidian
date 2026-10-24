//! Правки одной вкладки — остальным (US4, research.md §5).
//!
//! Бэкенд сообщает об изменениях на диске сам, но запись через него же — своя, и
//! внешним изменением для бэкенда не является. Для вкладок того же vault это чужая
//! правка: после успешной изменяющей команды сервер рассылает им то же событие
//! `VaultEvent`, какое пришло бы от внешней программы, — вкладка-источник его не
//! получает.

use serde_json::{json, Value};

use crate::vaults::OpenVault;

/// События, которые должны увидеть другие вкладки после команды `command` с
/// аргументами `arguments` (в camelCase, как их шлёт клиент).
pub fn echo(command: &str, arguments: &Value) -> Vec<Value> {
    let text = |key: &str| arguments.get(key).and_then(Value::as_str);
    match command {
        "write" | "write_board" => text("path")
            .map(|path| vec![json!({ "type": "NoteUpdated", "path": path })])
            .unwrap_or_default(),
        "create" | "create_board" => text("path")
            .map(|path| vec![json!({ "type": "NoteCreated", "path": path })])
            .unwrap_or_default(),
        "delete" => text("path")
            .map(|path| vec![json!({ "type": "NoteDeleted", "path": path })])
            .unwrap_or_default(),
        "rename" => match (text("from"), text("to")) {
            (Some(from), Some(to)) => {
                vec![json!({ "type": "NoteRenamed", "from_path": from, "to_path": to })]
            }
            _ => Vec::new(),
        },
        // Перенос папки задевает много файлов: та же обёртка, что у массового
        // изменения снаружи, — вкладка перечитает дерево целиком.
        "move_entry" => vec![
            json!({ "type": "BulkChangeStarted" }),
            json!({ "type": "BulkChangeSettled" }),
        ],
        _ => Vec::new(),
    }
}

pub fn send_echo(open: &OpenVault, events: Vec<Value>, origin: Option<String>) {
    let _ = open.events.send(std::sync::Arc::new(json!({
        "events": events,
        "origin": origin,
    })));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echoes_mutations_only() {
        assert_eq!(
            echo("write", &json!({ "path": "a.typ", "content": "x" })),
            vec![json!({ "type": "NoteUpdated", "path": "a.typ" })]
        );
        assert_eq!(
            echo("rename", &json!({ "from": "a.typ", "to": "b.typ" }))[0]["to_path"],
            "b.typ"
        );
        assert!(echo("read", &json!({ "path": "a.typ" })).is_empty());
        assert!(echo("write", &json!({})).is_empty());
    }
}
