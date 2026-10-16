//! cargo test — только native (принцип II). Парный файл — `graph_wasm.rs`.
//!
//! Оба прогоняют одну и ту же таблицу из `common::graph_cases`: правила графа
//! платформенно нейтральны. Кэш ссылок и вставок здесь — SQLite, тот же трейт
//! `IndexCache`, что у `MemoryCache` на wasm. Проверки `NativeBackend` и его
//! кэша на диске — ниже, только для native.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::graph_cases as cases;

#[tokio::test]
async fn nodes_and_edges_come_from_the_index() {
    cases::nodes_and_edges_come_from_the_index().await;
}

#[tokio::test]
async fn loops_and_broken_links_are_dropped() {
    cases::loops_and_broken_links_are_dropped().await;
}

#[tokio::test]
async fn a_pair_of_links_is_one_edge_with_both_ways() {
    cases::a_pair_of_links_is_one_edge_with_both_ways().await;
}

#[tokio::test]
async fn a_link_with_text_points_at_the_same_note() {
    cases::a_link_with_text_points_at_the_same_note().await;
}

#[tokio::test]
async fn orphans_can_be_hidden_and_degree_counts_the_visible_graph() {
    cases::orphans_can_be_hidden_and_degree_counts_the_visible_graph().await;
}

#[tokio::test]
async fn an_empty_vault_is_an_empty_graph() {
    cases::an_empty_vault_is_an_empty_graph().await;
}

#[tokio::test]
async fn a_rename_changes_the_id_and_keeps_the_links() {
    cases::a_rename_changes_the_id_and_keeps_the_links(
        typsidian_core::vault::cache::SqliteCache::in_memory()
            .await
            .unwrap(),
    )
    .await;
}

#[tokio::test]
async fn a_relocation_re_resolves_embeds() {
    cases::a_relocation_re_resolves_embeds(
        typsidian_core::vault::cache::SqliteCache::in_memory()
            .await
            .unwrap(),
    )
    .await;
}

#[test]
fn image_calls_are_read_with_the_typst_parser() {
    cases::image_calls_are_read_with_the_typst_parser();
}

#[tokio::test]
async fn attachments_are_shown_only_on_request() {
    cases::attachments_are_shown_only_on_request().await;
}

#[tokio::test]
async fn tags_come_from_properties() {
    cases::tags_come_from_properties().await;
}

#[tokio::test]
async fn the_filter_matches_text_in_name_and_path() {
    cases::the_filter_matches_text_in_name_and_path().await;
}

#[tokio::test]
async fn the_filter_understands_folder_and_tag_conditions() {
    cases::the_filter_understands_folder_and_tag_conditions().await;
}

#[tokio::test]
async fn the_filter_applies_to_attachments_and_tags() {
    cases::the_filter_applies_to_attachments_and_tags().await;
}

#[tokio::test]
async fn the_first_matching_group_wins() {
    cases::the_first_matching_group_wins().await;
}

#[tokio::test]
async fn groups_apply_to_tag_and_attachment_nodes() {
    cases::groups_apply_to_tag_and_attachment_nodes().await;
}

#[tokio::test]
async fn local_graph_follows_links_by_depth() {
    cases::local_graph_follows_links_by_depth().await;
}

#[tokio::test]
async fn local_graph_keeps_edges_between_reached_nodes() {
    cases::local_graph_keeps_edges_between_reached_nodes().await;
}

#[tokio::test]
async fn a_center_without_links_is_one_node() {
    cases::a_center_without_links_is_one_node().await;
}

#[tokio::test]
async fn a_center_outside_the_filter_stays() {
    cases::a_center_outside_the_filter_stays().await;
}

#[tokio::test]
async fn an_unknown_center_is_an_empty_graph() {
    cases::an_unknown_center_is_an_empty_graph().await;
}

#[tokio::test]
async fn a_tag_can_be_the_center() {
    cases::a_tag_can_be_the_center().await;
}

#[test]
fn the_json_shape_is_the_contract() {
    cases::the_json_shape_is_the_contract();
}

#[tokio::test]
async fn graph_settings_default_when_absent() {
    cases::graph_settings_default_when_absent().await;
}

#[tokio::test]
async fn graph_settings_round_trip_through_the_file() {
    cases::graph_settings_round_trip_through_the_file().await;
}

#[test]
fn graph_settings_tolerate_bad_values() {
    cases::graph_settings_tolerate_bad_values();
}

#[tokio::test]
async fn writing_graph_settings_keeps_other_keys() {
    cases::writing_graph_settings_keeps_other_keys().await;
}

#[tokio::test]
async fn graph_settings_are_normalized_on_write() {
    cases::graph_settings_are_normalized_on_write().await;
}

// ── NativeBackend: индекс и кэш на диске ────────────────────────────────────

mod native_index {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;
    use typsidian_core::graph::view::{vault_graph, GraphRequest};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    fn request() -> GraphRequest {
        GraphRequest {
            filter: String::new(),
            show_orphans: true,
            show_attachments: true,
            show_tags: false,
            groups: Vec::new(),
            local: None,
        }
    }

    fn ids(graph: &typsidian_core::graph::view::VaultGraph) -> Vec<String> {
        graph.nodes.iter().map(|node| node.id.clone()).collect()
    }

    fn vault_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("п")).unwrap();
        fs::write(dir.path().join("п/а.typ"), "[[/б]] #image(\"pic.png\")").unwrap();
        fs::write(dir.path().join("б.typ"), "[[/п/а]]").unwrap();
        fs::write(dir.path().join("доска.excalidraw"), "{}").unwrap();
        dir
    }

    /// Граф берётся из индекса: открытие наполняет ссылки и вставки, запись и
    /// переименование обновляют их в том же шаге.
    #[tokio::test]
    async fn the_graph_follows_the_native_index() {
        let dir = vault_dir();
        let mut backend = NativeBackend::open(dir.path()).await.unwrap();
        // Сверка с диском после открытия идёт в фоне и пишет в кэш то, что нашла
        // на момент обхода; запись теста обязана прийти после неё, иначе сверка
        // затёрла бы свежие ссылки устаревшими (так в приложении не бывает: правка
        // приходит позже открытия на секунды).
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let graph = vault_graph(&backend, &request()).await;
        assert_eq!(
            ids(&graph),
            ["б.typ", "доска.excalidraw", "п/pic.png", "п/а.typ"]
        );
        assert_eq!(graph.edges.len(), 2, "пара а<->б и вложение");
        assert!(graph.edges.iter().any(|edge| edge.both_ways));

        backend
            .write(Path::new("б.typ"), "[[/доска.excalidraw]]", None)
            .await
            .unwrap();
        let graph = vault_graph(&backend, &request()).await;
        assert!(edges_of(&graph, "б.typ", "доска.excalidraw"));
        assert!(!graph.edges.iter().any(|edge| edge.both_ways));

        backend
            .write(Path::new("п/а.typ"), "без картинки", None)
            .await
            .unwrap();
        let graph = vault_graph(&backend, &request()).await;
        assert!(!ids(&graph).contains(&"п/pic.png".to_string()));

        backend
            .rename(Path::new("б.typ"), Path::new("в.typ"))
            .await
            .unwrap();
        let graph = vault_graph(&backend, &request()).await;
        assert!(ids(&graph).contains(&"в.typ".to_string()));
        assert!(!ids(&graph).contains(&"б.typ".to_string()));
        assert!(edges_of(&graph, "в.typ", "доска.excalidraw"));

        backend.delete(Path::new("в.typ")).await.unwrap();
        let graph = vault_graph(&backend, &request()).await;
        assert!(graph.edges.is_empty());
    }

    fn edges_of(graph: &typsidian_core::graph::view::VaultGraph, from: &str, to: &str) -> bool {
        graph.edges.iter().any(|edge| {
            let (a, b) = (
                &graph.nodes[edge.from as usize].id,
                &graph.nodes[edge.to as usize].id,
            );
            (a == from && b == to) || (edge.both_ways && a == to && b == from)
        })
    }

    /// Вставки переживают повторное открытие (таблица `embeds` в SQLite).
    #[tokio::test]
    async fn embeds_survive_reopening() {
        let dir = vault_dir();
        {
            let backend = NativeBackend::open(dir.path()).await.unwrap();
            assert!(ids(&vault_graph(&backend, &request()).await).contains(&"п/pic.png".into()));
        }
        let backend = NativeBackend::open(dir.path()).await.unwrap();
        assert!(ids(&vault_graph(&backend, &request()).await).contains(&"п/pic.png".into()));
    }
}
