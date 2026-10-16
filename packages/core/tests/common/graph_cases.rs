//! Граф связей: таблица проверок для обоих таргетов (фича 025).
//!
//! Граф строится над `VaultBackend` и над `GraphSource`, поэтому правила
//! проверяются на `MemoryVault` и на готовых данных без файловой системы;
//! кэш ссылок и вставок — через трейт `IndexCache`, на который каждый раннер
//! подставляет свою реализацию (SQLite или память). Файлы `graph_native.rs` и
//! `graph_wasm.rs` только вызывают эти функции (принцип II).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use typsidian_core::graph::embeds_in;
use typsidian_core::graph::view::{
    build_graph, vault_graph, GraphForces, GraphGroup, GraphNode, GraphNodeKind, GraphRequest,
    GraphSettings, GraphSource, GroupColor, LocalGraph, VaultGraph,
};
use typsidian_core::properties::IndexedNote;
use typsidian_core::vault::cache::{CacheMove, IndexCache};
use typsidian_core::vault::vault_settings::{
    parse, read_vault_settings, render, write_vault_settings, VaultOverrides,
};

use super::MemoryVault;

// ── Помощники ───────────────────────────────────────────────────────────────

fn vault(files: &[(&str, &str)]) -> MemoryVault {
    files
        .iter()
        .fold(MemoryVault::new(), |vault, (path, text)| {
            vault.with_file(path, *text)
        })
}

fn everything() -> GraphRequest {
    GraphRequest {
        filter: String::new(),
        show_orphans: true,
        show_attachments: false,
        show_tags: false,
        groups: Vec::new(),
        local: None,
    }
}

fn with_filter(filter: &str) -> GraphRequest {
    GraphRequest {
        filter: filter.into(),
        ..everything()
    }
}

fn local(center: &str, depth: u8) -> GraphRequest {
    GraphRequest {
        local: Some(LocalGraph {
            center: center.into(),
            depth,
        }),
        ..everything()
    }
}

fn ids(graph: &VaultGraph) -> Vec<&str> {
    graph.nodes.iter().map(|node| node.id.as_str()).collect()
}

/// Связи как `(из, в, в обе стороны)` по идентификаторам узлов.
fn edges(graph: &VaultGraph) -> Vec<(&str, &str, bool)> {
    graph
        .edges
        .iter()
        .map(|edge| {
            (
                graph.nodes[edge.from as usize].id.as_str(),
                graph.nodes[edge.to as usize].id.as_str(),
                edge.both_ways,
            )
        })
        .collect()
}

fn node<'a>(graph: &'a VaultGraph, id: &str) -> &'a GraphNode {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .unwrap_or_else(|| panic!("узла {id} нет: {:?}", ids(graph)))
}

const TAGGED: &str = "#metadata((tags: (\"проект\", \"срочно\"))) <properties>\n";

// ── Узлы и связи (T008) ─────────────────────────────────────────────────────

/// Узлы по возрастанию `id`, связи из индекса, число связей, виды, итог.
pub async fn nodes_and_edges_come_from_the_index() {
    let v = vault(&[
        ("b.typ", "[[c]] и [[доска.excalidraw]]"),
        ("a.typ", "[[b]]"),
        ("c.typ", "пусто"),
        ("доска.excalidraw", "{}"),
        ("картинка.png", "не заметка"),
    ]);
    let graph = vault_graph(&v, &everything()).await;
    assert_eq!(ids(&graph), ["a.typ", "b.typ", "c.typ", "доска.excalidraw"]);
    assert_eq!(graph.total_notes, 4);
    assert_eq!(graph.center, None);
    assert_eq!(
        edges(&graph),
        [
            ("a.typ", "b.typ", false),
            ("b.typ", "c.typ", false),
            ("b.typ", "доска.excalidraw", false),
        ]
    );
    assert_eq!(node(&graph, "b.typ").degree, 3);
    assert_eq!(node(&graph, "a.typ").degree, 1);
    assert_eq!(node(&graph, "b.typ").kind, GraphNodeKind::Note);
    assert_eq!(node(&graph, "b.typ").label, "b");
    assert_eq!(node(&graph, "b.typ").path.as_deref(), Some("b.typ"));
    let board = node(&graph, "доска.excalidraw");
    assert_eq!(board.kind, GraphNodeKind::Board);
    assert_eq!(board.label, "доска");
    // Результат не зависит от порядка вызова.
    assert_eq!(graph, vault_graph(&v, &everything()).await);
}

/// Петля и ссылка в никуда связей не дают, узла-призрака нет.
pub async fn loops_and_broken_links_are_dropped() {
    let v = vault(&[("a.typ", "[[a]] [[нет такой]] [[b]]"), ("b.typ", "")]);
    let graph = vault_graph(&v, &everything()).await;
    assert_eq!(ids(&graph), ["a.typ", "b.typ"]);
    assert_eq!(edges(&graph), [("a.typ", "b.typ", false)]);
    assert_eq!(node(&graph, "a.typ").degree, 1);
}

/// Пара ссылок в обе стороны — одна связь с `both_ways`; в одну сторону —
/// направление от ссылающейся к цели.
pub async fn a_pair_of_links_is_one_edge_with_both_ways() {
    let v = vault(&[
        ("a.typ", "[[b]]"),
        ("b.typ", "[[a]] [[c]]"),
        ("c.typ", ""),
        ("d.typ", "[[a]] [[a]]"),
    ]);
    let graph = vault_graph(&v, &everything()).await;
    assert_eq!(
        edges(&graph),
        [
            ("a.typ", "b.typ", true),
            ("b.typ", "c.typ", false),
            ("d.typ", "a.typ", false),
        ]
    );
    assert_eq!(node(&graph, "a.typ").degree, 2);
}

/// `[[путь|текст]]` (021) ведёт к той же цели, что и `[[путь]]`.
pub async fn a_link_with_text_points_at_the_same_note() {
    let v = vault(&[
        ("a.typ", "[[/папка/b|ссылка с текстом]]"),
        ("папка/b.typ", ""),
    ]);
    let graph = vault_graph(&v, &everything()).await;
    assert_eq!(edges(&graph), [("a.typ", "папка/b.typ", false)]);
}

/// Заметки без связей: показ включён — они есть, выключен — нет; число связей
/// считается по видимому графу.
pub async fn orphans_can_be_hidden_and_degree_counts_the_visible_graph() {
    let v = vault(&[("a.typ", "[[b]]"), ("b.typ", ""), ("одна.typ", "")]);
    let shown = vault_graph(&v, &everything()).await;
    assert_eq!(ids(&shown), ["a.typ", "b.typ", "одна.typ"]);
    assert_eq!(node(&shown, "одна.typ").degree, 0);
    assert_eq!(shown.total_notes, 3);

    let hidden = vault_graph(
        &v,
        &GraphRequest {
            show_orphans: false,
            ..everything()
        },
    )
    .await;
    assert_eq!(ids(&hidden), ["a.typ", "b.typ"]);
    assert_eq!(hidden.total_notes, 3, "итог — до фильтра");

    // Фильтр оставляет только `a`: у неё не остаётся связей, и без показа
    // одиночек граф пуст.
    let filtered = vault_graph(
        &v,
        &GraphRequest {
            filter: "a.typ".into(),
            show_orphans: false,
            ..everything()
        },
    )
    .await;
    assert!(filtered.nodes.is_empty());
}

/// Пустой vault — пустой граф.
pub async fn an_empty_vault_is_an_empty_graph() {
    let graph = vault_graph(&MemoryVault::new(), &everything()).await;
    assert!(graph.nodes.is_empty() && graph.edges.is_empty());
    assert_eq!(graph.total_notes, 0);
}

/// Кэш хранит ссылки и вставки; переименование, переезд и удаление заметки
/// уносят их с собой, а граф по кэшу меняет `id` узла и сохраняет связи.
pub async fn a_rename_changes_the_id_and_keeps_the_links<C: IndexCache>(mut cache: C) {
    let a = Path::new("а.typ");
    let b = Path::new("б.typ");
    let renamed = Path::new("новая.typ");
    cache.set_links(a, &[b.to_path_buf()]).await;
    cache.set_embeds(a, &[PathBuf::from("pic.png")]).await;
    assert_eq!(cache.all_links().await.get(a), Some(&vec![b.to_path_buf()]));
    assert_eq!(
        cache.all_embeds().await.get(a),
        Some(&vec![PathBuf::from("pic.png")])
    );

    cache.rename_note(a, renamed).await;
    let links = cache.all_links().await;
    assert!(!links.contains_key(a));
    assert_eq!(links.get(renamed), Some(&vec![b.to_path_buf()]));

    let source = GraphSource {
        notes: vec![
            IndexedNote::new(renamed, "новая.typ", None),
            IndexedNote::new(b, "б.typ", None),
        ],
        boards: Vec::new(),
        links,
        embeds: cache.all_embeds().await,
    };
    let request = GraphRequest {
        show_attachments: true,
        ..everything()
    };
    let graph = build_graph(&source, &request, 0);
    assert_eq!(ids(&graph), ["pic.png", "б.typ", "новая.typ"]);
    assert_eq!(
        edges(&graph),
        [
            ("новая.typ", "pic.png", false),
            ("новая.typ", "б.typ", false)
        ]
    );

    // Пустой список вставок стирает запись.
    cache.set_embeds(renamed, &[]).await;
    assert!(cache.all_embeds().await.is_empty());
    cache.set_embeds(renamed, &[PathBuf::from("x.png")]).await;
    cache.remove_note(renamed).await;
    assert!(cache.all_embeds().await.is_empty());
    assert!(cache.all_links().await.is_empty());
}

/// Переезд заметки: вставки пересчитываются под новую папку, ссылки — тоже.
pub async fn a_relocation_re_resolves_embeds<C: IndexCache>(mut cache: C) {
    let from = PathBuf::from("A/x.typ");
    let to = PathBuf::from("B/x.typ");
    cache.set_embeds(&from, &[PathBuf::from("A/pic.png")]).await;
    cache
        .relocate(&[CacheMove {
            from: from.clone(),
            to: to.clone(),
            links: Some(Vec::new()),
            embeds: Some(vec![PathBuf::from("B/pic.png")]),
        }])
        .await;
    let embeds = cache.all_embeds().await;
    assert!(!embeds.contains_key(&from));
    assert_eq!(embeds.get(&to), Some(&vec![PathBuf::from("B/pic.png")]));

    // Без пересчёта записи только переезжают.
    cache
        .relocate(&[CacheMove {
            from: to.clone(),
            to: from.clone(),
            links: None,
            embeds: None,
        }])
        .await;
    assert_eq!(
        cache.all_embeds().await.get(&from),
        Some(&vec![PathBuf::from("B/pic.png")])
    );
}

// ── Вложения и теги (T020) ──────────────────────────────────────────────────

/// Разбор `#image("…")`: путь от папки заметки или от корня, вложенные вызовы,
/// дубли схлопываются, не литерал и чужие функции пропускаются.
pub fn image_calls_are_read_with_the_typst_parser() {
    let text = concat!(
        "#image(\"a.png\")\n",
        "#figure(image(\"../общее/b.svg\", width: 50%), caption: [x])\n",
        "#image(\"/корень/c.png\")\n",
        "#image(\"a.png\")\n",
        "#imagemap(\"нет.png\")\n",
        "#image(bytes)\n",
        "#image(\"https://example.org/d.png\")\n",
        "#let x = \"#image(\\\"в строке.png\\\")\"\n",
        "#image(\"../../наружу.png\")\n",
    );
    assert_eq!(
        embeds_in(Path::new("папка/заметка.typ"), text),
        [
            PathBuf::from("папка/a.png"),
            PathBuf::from("общее/b.svg"),
            PathBuf::from("корень/c.png"),
        ]
    );
    assert!(embeds_in(Path::new("a.typ"), "просто текст").is_empty());
    // Битый Typst не роняет разбор.
    let _ = embeds_in(Path::new("a.typ"), "#image(\"a.png\"");
}

/// Вложения: картинка и снимок доски; без включения узлов нет, общий файл —
/// один узел.
pub async fn attachments_are_shown_only_on_request() {
    let v = vault(&[
        (
            "a.typ",
            "#image(\"pic.png\")\n#image(\"assets/a/board-ab12cd.svg\")",
        ),
        ("b.typ", "#figure(image(\"pic.png\"))"),
        ("c.typ", "#image(bytes)"),
    ]);
    let off = vault_graph(&v, &everything()).await;
    assert_eq!(ids(&off), ["a.typ", "b.typ", "c.typ"]);
    assert!(off.edges.is_empty());

    let on = vault_graph(
        &v,
        &GraphRequest {
            show_attachments: true,
            ..everything()
        },
    )
    .await;
    assert_eq!(
        ids(&on),
        [
            "a.typ",
            "assets/a/board-ab12cd.svg",
            "b.typ",
            "c.typ",
            "pic.png"
        ]
    );
    let pic = node(&on, "pic.png");
    assert_eq!(pic.kind, GraphNodeKind::Attachment);
    assert_eq!(pic.degree, 2);
    assert_eq!(pic.label, "pic.png");
    assert_eq!(pic.path.as_deref(), Some("pic.png"));
    assert_eq!(on.edges.len(), 3);
    // Заметка без связей по-прежнему видна как одиночка...
    assert_eq!(node(&on, "c.typ").degree, 0);
    // ...и пропадает вместе с ними, если одиночек прячут; вложения остаются.
    let hidden = vault_graph(
        &v,
        &GraphRequest {
            show_attachments: true,
            show_orphans: false,
            ..everything()
        },
    )
    .await;
    assert!(!ids(&hidden).contains(&"c.typ"));
    assert!(ids(&hidden).contains(&"pic.png"));
}

/// Теги из свойств: узел на тег, общий тег — один узел; регистр и `#` не
/// различают; без включения тегов нет.
pub async fn tags_come_from_properties() {
    let v = vault(&[
        (
            "a.typ",
            "#metadata((tags: (\"Проект\", \"срочно\"))) <properties>\n",
        ),
        ("b.typ", "#metadata((tags: (\"#проект\",))) <properties>\n"),
        ("c.typ", "без тегов"),
    ]);
    let off = vault_graph(&v, &everything()).await;
    assert_eq!(ids(&off), ["a.typ", "b.typ", "c.typ"]);

    let on = vault_graph(
        &v,
        &GraphRequest {
            show_tags: true,
            ..everything()
        },
    )
    .await;
    assert_eq!(
        ids(&on),
        ["a.typ", "b.typ", "c.typ", "tag:проект", "tag:срочно"]
    );
    let tag = node(&on, "tag:проект");
    assert_eq!(tag.kind, GraphNodeKind::Tag);
    assert_eq!(tag.path, None);
    assert_eq!(tag.degree, 2);
    assert_eq!(on.edges.len(), 3);
    assert!(edges(&on).contains(&("a.typ", "tag:проект", false)));
}

// ── Фильтр и группы (T020) ──────────────────────────────────────────────────

fn projects() -> MemoryVault {
    vault(&[
        (
            "проекты/план.typ",
            "#metadata((tags: (\"проект\",))) <properties>\n[[идея]]",
        ),
        ("проекты/идея.typ", "[[/заметка]]"),
        ("заметка.typ", "[[/проекты/план]]"),
        ("архив/старое.typ", "[[/проекты/идея]]"),
    ])
}

/// Фильтр-текст ищется в имени и пути без учёта регистра; связь остаётся,
/// только если подошли оба конца.
pub async fn the_filter_matches_text_in_name_and_path() {
    let v = projects();
    let by_name = vault_graph(&v, &with_filter("ПЛАН")).await;
    assert_eq!(ids(&by_name), ["проекты/план.typ"]);
    assert!(by_name.edges.is_empty());
    assert_eq!(by_name.total_notes, 4);

    let by_path = vault_graph(&v, &with_filter("проекты")).await;
    assert_eq!(ids(&by_path), ["проекты/идея.typ", "проекты/план.typ"]);
    assert_eq!(
        edges(&by_path),
        [("проекты/план.typ", "проекты/идея.typ", false)]
    );
    assert_eq!(node(&by_path, "проекты/идея.typ").degree, 1);

    // Несколько слов — все должны встретиться.
    let both = vault_graph(&v, &with_filter("проекты идея")).await;
    assert_eq!(ids(&both), ["проекты/идея.typ"]);
    assert!(vault_graph(&v, &with_filter("такого нет"))
        .await
        .nodes
        .is_empty());
}

/// `папка:` и `тег:` — те же условия, что в поиске 020.
pub async fn the_filter_understands_folder_and_tag_conditions() {
    let v = projects();
    let folder = vault_graph(&v, &with_filter("папка:проекты")).await;
    assert_eq!(ids(&folder), ["проекты/идея.typ", "проекты/план.typ"]);
    let folder_en = vault_graph(&v, &with_filter("folder:архив")).await;
    assert_eq!(ids(&folder_en), ["архив/старое.typ"]);

    let tag = vault_graph(&v, &with_filter("тег:проект")).await;
    assert_eq!(ids(&tag), ["проекты/план.typ"]);
    let tag_en = vault_graph(&v, &with_filter("tag:Проект")).await;
    assert_eq!(ids(&tag_en), ["проекты/план.typ"]);

    // Условие и текст вместе.
    let mixed = vault_graph(&v, &with_filter("папка:проекты идея")).await;
    assert_eq!(ids(&mixed), ["проекты/идея.typ"]);
}

/// Фильтр действует и на вложения и теги: узел-тег проходит по `тег:` с тем же
/// именем и по тексту; `папка:` его отсекает.
pub async fn the_filter_applies_to_attachments_and_tags() {
    let v = vault(&[
        (
            "п/а.typ",
            "#metadata((tags: (\"проект\",))) <properties>\n#image(\"pic.png\")",
        ),
        ("б.typ", "#image(\"pic.png\")"),
    ]);
    let request = |filter: &str| GraphRequest {
        filter: filter.into(),
        show_tags: true,
        show_attachments: true,
        ..everything()
    };
    let all = vault_graph(&v, &request("")).await;
    assert_eq!(all.nodes.len(), 5, "{:?}", ids(&all));

    let tagged = vault_graph(&v, &request("тег:проект")).await;
    assert_eq!(ids(&tagged), ["tag:проект", "п/а.typ"]);
    assert_eq!(edges(&tagged), [("п/а.typ", "tag:проект", false)]);

    let folder = vault_graph(&v, &request("папка:п")).await;
    assert_eq!(ids(&folder), ["п/pic.png", "п/а.typ"]);

    let text = vault_graph(&v, &request("pic")).await;
    // Заметки не подошли — вложения остались без связей и узлами не бывают.
    assert!(text.nodes.is_empty());
}

/// Группа — индекс первой подошедшей; пустое условие группы не красит никого.
pub async fn the_first_matching_group_wins() {
    let v = projects();
    let groups = vec![
        GraphGroup {
            query: "папка:проекты".into(),
            color: GroupColor::Blue,
        },
        GraphGroup {
            query: "тег:проект".into(),
            color: GroupColor::Orange,
        },
        GraphGroup {
            query: "  ".into(),
            color: GroupColor::Red,
        },
        GraphGroup {
            query: "старое".into(),
            color: GroupColor::Green,
        },
    ];
    let graph = vault_graph(
        &v,
        &GraphRequest {
            groups: groups.clone(),
            ..everything()
        },
    )
    .await;
    // `план` подходит под обе первые группы — достаётся первая.
    assert_eq!(node(&graph, "проекты/план.typ").group, Some(0));
    assert_eq!(node(&graph, "проекты/идея.typ").group, Some(0));
    assert_eq!(node(&graph, "архив/старое.typ").group, Some(3));
    assert_eq!(node(&graph, "заметка.typ").group, None);

    // Порядок групп решает: тег стал первым.
    let reordered = vault_graph(
        &v,
        &GraphRequest {
            groups: vec![groups[1].clone(), groups[0].clone()],
            ..everything()
        },
    )
    .await;
    assert_eq!(node(&reordered, "проекты/план.typ").group, Some(0));
    assert_eq!(node(&reordered, "проекты/идея.typ").group, Some(1));
}

/// Группа окрашивает и теги, и вложения по их собственным сведениям.
pub async fn groups_apply_to_tag_and_attachment_nodes() {
    let v = vault(&[(
        "а.typ",
        "#metadata((tags: (\"проект\",))) <properties>\n#image(\"img/pic.png\")",
    )]);
    let graph = vault_graph(
        &v,
        &GraphRequest {
            show_tags: true,
            show_attachments: true,
            groups: vec![
                GraphGroup {
                    query: "тег:проект".into(),
                    color: GroupColor::Amber,
                },
                GraphGroup {
                    query: "папка:img".into(),
                    color: GroupColor::Steel,
                },
            ],
            ..everything()
        },
    )
    .await;
    assert_eq!(node(&graph, "tag:проект").group, Some(0));
    assert_eq!(node(&graph, "а.typ").group, Some(0));
    assert_eq!(node(&graph, "img/pic.png").group, Some(1));
}

// ── Локальный граф (T017) ───────────────────────────────────────────────────

fn chain() -> MemoryVault {
    // a - b - c - d, и e ссылается на b; z — одиночка.
    vault(&[
        ("a.typ", "[[b]]"),
        ("b.typ", "[[c]]"),
        ("c.typ", "[[d]]"),
        ("d.typ", ""),
        ("e.typ", "[[b]]"),
        ("z.typ", ""),
    ])
}

/// Глубина 1, 2, 3; связи берутся в обе стороны.
pub async fn local_graph_follows_links_by_depth() {
    let v = chain();
    let one = vault_graph(&v, &local("b.typ", 1)).await;
    assert_eq!(ids(&one), ["a.typ", "b.typ", "c.typ", "e.typ"]);
    assert_eq!(
        one.center.map(|i| one.nodes[i as usize].id.as_str()),
        Some("b.typ")
    );
    assert_eq!(node(&one, "b.typ").degree, 3);
    assert_eq!(one.total_notes, 6);

    let two = vault_graph(&v, &local("b.typ", 2)).await;
    assert_eq!(ids(&two), ["a.typ", "b.typ", "c.typ", "d.typ", "e.typ"]);

    // От края: ссылка на `b` ведёт обратно по входящим.
    let from_a = vault_graph(&v, &local("a.typ", 2)).await;
    assert_eq!(ids(&from_a), ["a.typ", "b.typ", "c.typ", "e.typ"]);
    let from_a3 = vault_graph(&v, &local("a.typ", 3)).await;
    assert_eq!(ids(&from_a3), ["a.typ", "b.typ", "c.typ", "d.typ", "e.typ"]);
    // От `d` вспять: c, b; e и a — на третьем шаге.
    let from_d = vault_graph(&v, &local("d.typ", 2)).await;
    assert_eq!(ids(&from_d), ["b.typ", "c.typ", "d.typ"]);
    assert_eq!(vault_graph(&v, &local("d.typ", 3)).await.nodes.len(), 5);

    // Глубина вне 1..=3 приводится к границе.
    assert_eq!(vault_graph(&v, &local("a.typ", 0)).await.nodes.len(), 2);
    assert_eq!(vault_graph(&v, &local("a.typ", 9)).await.nodes.len(), 5);
}

/// Локальный граф — подграф: связь между двумя достигнутыми узлами остаётся.
pub async fn local_graph_keeps_edges_between_reached_nodes() {
    let v = vault(&[("a.typ", "[[b]] [[c]]"), ("b.typ", "[[c]]"), ("c.typ", "")]);
    let graph = vault_graph(&v, &local("a.typ", 1)).await;
    assert_eq!(graph.edges.len(), 3);
}

/// Центр без связей — один узел, даже при скрытых одиночках.
pub async fn a_center_without_links_is_one_node() {
    let v = chain();
    let graph = vault_graph(
        &v,
        &GraphRequest {
            show_orphans: false,
            ..local("z.typ", 2)
        },
    )
    .await;
    assert_eq!(ids(&graph), ["z.typ"]);
    assert_eq!(graph.center, Some(0));
    assert!(graph.edges.is_empty());
    assert_eq!(graph.nodes[0].degree, 0);
}

/// Центр, не прошедший фильтр, остаётся; обход идёт по тому, что прошло.
pub async fn a_center_outside_the_filter_stays() {
    let v = chain();
    // Фильтру `c.typ` не соответствует ни `b`, ни `d`, кроме самого `c`.
    let graph = vault_graph(
        &v,
        &GraphRequest {
            filter: "c.typ".into(),
            ..local("b.typ", 2)
        },
    )
    .await;
    assert_eq!(ids(&graph), ["b.typ", "c.typ"]);
    assert_eq!(edges(&graph), [("b.typ", "c.typ", false)]);
    assert!(graph.center.is_some());

    // Фильтру не соответствует никто, кроме центра: он один.
    let alone = vault_graph(
        &v,
        &GraphRequest {
            filter: "z.typ".into(),
            ..local("b.typ", 3)
        },
    )
    .await;
    assert_eq!(ids(&alone), ["b.typ"]);
}

/// Центра нет в графе — граф пуст, но итог по vault есть.
pub async fn an_unknown_center_is_an_empty_graph() {
    let graph = vault_graph(&chain(), &local("нет.typ", 1)).await;
    assert!(graph.nodes.is_empty() && graph.edges.is_empty());
    assert_eq!(graph.center, None);
    assert_eq!(graph.total_notes, 6);
}

/// Центр — и вложение, и тег, если их показ включён.
pub async fn a_tag_can_be_the_center() {
    let v = vault(&[
        ("а.typ", "#metadata((tags: (\"т\",))) <properties>\n"),
        ("б.typ", "#metadata((tags: (\"т\",))) <properties>\n"),
        ("в.typ", ""),
    ]);
    let graph = vault_graph(
        &v,
        &GraphRequest {
            show_tags: true,
            ..local("tag:т", 1)
        },
    )
    .await;
    assert_eq!(ids(&graph), ["tag:т", "а.typ", "б.typ"]);
}

// ── Форма для интерфейса ────────────────────────────────────────────────────

/// JSON-форма запроса и ответа — контракт с интерфейсом.
pub fn the_json_shape_is_the_contract() {
    let request = GraphRequest {
        filter: "тег:x".into(),
        show_orphans: true,
        show_attachments: false,
        show_tags: true,
        groups: vec![GraphGroup {
            query: "папка:a".into(),
            color: GroupColor::Hivis,
        }],
        local: Some(LocalGraph {
            center: "a.typ".into(),
            depth: 2,
        }),
    };
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        serde_json::json!({
            "filter": "тег:x",
            "show_orphans": true,
            "show_attachments": false,
            "show_tags": true,
            "groups": [{"query": "папка:a", "color": "hivis"}],
            "local": {"center": "a.typ", "depth": 2},
        })
    );
    // `local` можно не передавать.
    let parsed: GraphRequest = serde_json::from_value(serde_json::json!({
        "filter": "", "show_orphans": true, "show_attachments": false,
        "show_tags": false, "groups": [],
    }))
    .unwrap();
    assert_eq!(parsed.local, None);

    let graph = build_graph(
        &GraphSource {
            notes: vec![IndexedNote::new(Path::new("a.typ"), "a.typ", None)],
            boards: vec![IndexedNote::new(
                Path::new("д.excalidraw"),
                "д.excalidraw",
                None,
            )],
            ..GraphSource::default()
        },
        &everything(),
        0,
    );
    assert_eq!(
        serde_json::to_value(&graph).unwrap(),
        serde_json::json!({
            "nodes": [
                {"id": "a.typ", "kind": "Note", "label": "a", "path": "a.typ", "degree": 0, "group": null},
                {"id": "д.excalidraw", "kind": "Board", "label": "д", "path": "д.excalidraw", "degree": 0, "group": null},
            ],
            "edges": [],
            "center": null,
            "total_notes": 2,
        })
    );
    let edge = serde_json::to_value(typsidian_core::graph::view::GraphEdge {
        from: 0,
        to: 1,
        both_ways: true,
    })
    .unwrap();
    assert_eq!(
        edge,
        serde_json::json!({"from": 0, "to": 1, "both_ways": true})
    );
}

// ── Настройки графа (T006, T007) ────────────────────────────────────────────

/// Умолчания: ключа нет — `None`, а умолчания ядра заданы значениями плана.
pub async fn graph_settings_default_when_absent() {
    let v = MemoryVault::new();
    assert_eq!(read_vault_settings(&v).await.unwrap().graph, None);
    assert_eq!(parse("{}").unwrap().graph, None);

    let defaults = GraphSettings::default();
    assert_eq!(defaults.filter, "");
    assert!(defaults.show_orphans);
    assert!(!defaults.show_attachments && !defaults.show_tags && !defaults.arrows);
    assert!(defaults.groups.is_empty());
    assert_eq!(
        defaults.forces,
        GraphForces {
            center: 0.3,
            repel: 0.6,
            link: 0.5,
            distance: 0.45
        }
    );
    assert_eq!(defaults.label_zoom, 0.4);
    assert_eq!(defaults.node_size, 0.5);

    // Пустая таблица — все умолчания, а не отсутствие.
    assert_eq!(
        parse("{\"graph\": {}}").unwrap().graph,
        Some(defaults.clone())
    );
    // Запрос по настройкам несёт фильтр, показ и группы; окно — отдельно.
    let request = defaults.request(Some(LocalGraph {
        center: "a.typ".into(),
        depth: 1,
    }));
    assert!(request.show_orphans && request.local.is_some());
}

/// Запись и чтение через файл vault возвращают те же настройки.
pub async fn graph_settings_round_trip_through_the_file() {
    let mut v = MemoryVault::new();
    let written = VaultOverrides {
        graph: Some(GraphSettings {
            filter: "папка:проекты".into(),
            show_orphans: false,
            show_attachments: true,
            show_tags: true,
            arrows: true,
            groups: vec![
                GraphGroup {
                    query: "папка:проекты".into(),
                    color: GroupColor::Orange,
                },
                GraphGroup {
                    query: "тег:срочно".into(),
                    color: GroupColor::Red,
                },
            ],
            forces: GraphForces {
                center: 0.25,
                repel: 0.75,
                link: 0.125,
                distance: 1.0,
            },
            label_zoom: 0.0,
            node_size: 0.875,
        }),
        ..VaultOverrides::default()
    };
    write_vault_settings(&mut v, &written).await.unwrap();
    assert_eq!(read_vault_settings(&v).await.unwrap(), written);
}

/// Негодные значения читаются как умолчание, числа — в пределах 0..=1, группа
/// с неизвестным цветом отбрасывается; файл остаётся читаемым.
pub fn graph_settings_tolerate_bad_values() {
    let text = r##"{
        "graph": {
            "filter": 5,
            "show_orphans": "да",
            "show_tags": true,
            "arrows": null,
            "groups": [
                {"query": "a", "color": "orange"},
                {"query": "b", "color": "пурпур"},
                {"query": "  ", "color": "blue"},
                "мусор"
            ],
            "forces": {"center": 7, "repel": -3, "link": "много"},
            "label_zoom": "x",
            "node_size": 0.3,
            "лишнее": 1
        }
    }"##;
    let graph = parse(text).expect("файл читается").graph.unwrap();
    let defaults = GraphSettings::default();
    assert_eq!(graph.filter, "");
    assert_eq!(graph.show_orphans, defaults.show_orphans);
    assert!(graph.show_tags);
    assert_eq!(graph.arrows, defaults.arrows);
    assert_eq!(
        graph.groups,
        [GraphGroup {
            query: "a".into(),
            color: GroupColor::Orange
        }]
    );
    assert_eq!(graph.forces.center, 1.0);
    assert_eq!(graph.forces.repel, 0.0);
    assert_eq!(graph.forces.link, defaults.forces.link);
    assert_eq!(graph.forces.distance, defaults.forces.distance);
    assert_eq!(graph.label_zoom, defaults.label_zoom);
    assert_eq!(graph.node_size, 0.3);

    // Не объект — как если бы ключа не было; остальные настройки читаются.
    let overrides = parse(r#"{"graph": 5, "settings": {"theme": "dark"}}"#).unwrap();
    assert_eq!(overrides.graph, None);
    assert!(overrides.theme.is_some());
}

/// Запись графа не трогает прочие ключи файла, в том числе чужие ключи внутри
/// самой таблицы `graph`; сброс (`None`) убирает только известные ключи.
pub async fn writing_graph_settings_keeps_other_keys() {
    let mut v = vault(&[(
        ".typsidian/vault.json",
        r##"{
  "lines": {"mine": 1},
  "assets": {"placement": "vault-root"},
  "graph": {"mine": "keep", "arrows": true}
}"##,
    )]);
    let mut overrides = read_vault_settings(&v).await.unwrap();
    assert!(overrides.graph.as_ref().unwrap().arrows);
    overrides.graph = Some(GraphSettings {
        show_tags: true,
        ..overrides.graph.take().unwrap()
    });
    write_vault_settings(&mut v, &overrides).await.unwrap();

    let file: serde_json::Value =
        serde_json::from_slice(&v.bytes_of(".typsidian/vault.json").expect("файл записан"))
            .unwrap();
    assert_eq!(file["lines"], serde_json::json!({"mine": 1}));
    assert_eq!(file["assets"]["placement"], "vault-root");
    assert_eq!(file["graph"]["mine"], "keep");
    assert_eq!(file["graph"]["show_tags"], true);
    assert_eq!(file["graph"]["arrows"], true);

    overrides.graph = None;
    write_vault_settings(&mut v, &overrides).await.unwrap();
    let file: serde_json::Value =
        serde_json::from_slice(&v.bytes_of(".typsidian/vault.json").expect("файл записан"))
            .unwrap();
    assert_eq!(file["graph"], serde_json::json!({"mine": "keep"}));
    assert_eq!(file["lines"], serde_json::json!({"mine": 1}));

    // Без чужих ключей пустая таблица убирается целиком.
    let rendered = render(r#"{"graph": {"arrows": true}}"#, &VaultOverrides::default());
    assert!(!rendered.contains("graph"), "{rendered}");
}

/// Запись приводит числа к допустимым пределам и не пишет пустые группы.
pub async fn graph_settings_are_normalized_on_write() {
    let mut v = MemoryVault::new();
    let overrides = VaultOverrides {
        graph: Some(GraphSettings {
            forces: GraphForces {
                center: 4.0,
                repel: f64::NAN,
                link: -1.0,
                distance: 0.5,
            },
            groups: vec![
                GraphGroup {
                    query: " ".into(),
                    color: GroupColor::Blue,
                },
                GraphGroup {
                    query: "x".into(),
                    color: GroupColor::Green,
                },
            ],
            ..GraphSettings::default()
        }),
        ..VaultOverrides::default()
    };
    write_vault_settings(&mut v, &overrides).await.unwrap();
    let graph = read_vault_settings(&v).await.unwrap().graph.unwrap();
    assert_eq!(graph.forces.center, 1.0);
    assert_eq!(graph.forces.repel, GraphForces::default().repel);
    assert_eq!(graph.forces.link, 0.0);
    assert_eq!(graph.groups.len(), 1);
}
