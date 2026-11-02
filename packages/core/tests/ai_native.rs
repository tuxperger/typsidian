//! cargo test — только native (принцип II). Парный файл — `ai_wasm.rs`.
#![cfg(not(target_arch = "wasm32"))]

mod common;

use common::ai_cases as cases;
use common::lesson_cases as lesson;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use typsidian_core::ai::transport::{AiTransport, DefaultTransport, HttpMethod, HttpRequest};
use typsidian_core::ai::{protocol::check_status, AiFailure};

#[tokio::test]
async fn anthropic_gets_a_default_model() {
    cases::anthropic_gets_a_default_model().await;
}

#[tokio::test]
async fn missing_or_broken_settings_mean_disabled() {
    cases::missing_or_broken_settings_mean_disabled().await;
}

#[tokio::test]
async fn disabled_makes_no_requests() {
    cases::disabled_makes_no_requests().await;
}

#[tokio::test]
async fn journal_gets_request_before_sending_and_never_the_key() {
    cases::journal_gets_request_before_sending_and_never_the_key().await;
}

#[tokio::test]
async fn journal_failure_stops_the_request() {
    cases::journal_failure_stops_the_request().await;
}

#[tokio::test]
async fn unknown_price_is_not_zero() {
    cases::unknown_price_is_not_zero().await;
}

#[tokio::test]
async fn failure_is_recorded_and_carries_no_server_text() {
    cases::failure_is_recorded_and_carries_no_server_text().await;
}

#[tokio::test]
async fn statuses_map_to_failures() {
    cases::statuses_map_to_failures().await;
}

#[tokio::test]
async fn journal_keeps_last_entries_and_clears() {
    cases::journal_keeps_last_entries_and_clears().await;
}

#[tokio::test]
async fn journal_stays_out_of_sync_and_settings_travel() {
    cases::journal_stays_out_of_sync_and_settings_travel().await;
}

#[tokio::test]
async fn endpoints_are_guarded() {
    cases::endpoints_are_guarded().await;
}

#[tokio::test]
async fn cloud_key_never_goes_to_custom_endpoint() {
    cases::cloud_key_never_goes_to_custom_endpoint().await;
}

#[tokio::test]
async fn models_are_listed_sorted() {
    cases::models_are_listed_sorted().await;
}

#[tokio::test]
async fn access_stays_inside_the_vault() {
    cases::access_stays_inside_the_vault().await;
}

#[tokio::test]
async fn access_ignores_unicode_form() {
    cases::access_ignores_unicode_form().await;
}

#[tokio::test]
async fn duplicate_request_id_keeps_the_first_cancellable() {
    cases::duplicate_request_id_keeps_the_first_cancellable().await;
}

#[tokio::test]
async fn custom_endpoint_needs_approval_on_this_device() {
    cases::custom_endpoint_needs_approval_on_this_device().await;
}

#[tokio::test]
async fn denied_folder_follows_the_move() {
    cases::denied_folder_follows_the_move().await;
}

#[tokio::test]
async fn cancel_stops_waiting_at_once() {
    cases::cancel_stops_waiting_at_once().await;
}

fn request(url: String) -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Post,
        url,
        headers: vec![("x-api-key".to_owned(), "test-key".to_owned())],
        body: Some("{}".to_owned()),
    }
}

/// Отказ соединения: понятный отказ, а не зависание (принцип VII).
#[tokio::test]
async fn refused_connection_is_offline() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let failure = DefaultTransport
        .send(request(format!("http://127.0.0.1:{port}/v1")))
        .await
        .unwrap_err();
    assert_eq!(failure, AiFailure::Offline);
}

/// FR-009: перенаправление не выполняется, второго соединения нет.
#[tokio::test]
async fn redirect_is_not_followed() {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target.local_addr().unwrap().port();
    let origin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin_port = origin.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut socket, _) = origin.accept().await.unwrap();
        let mut buffer = [0u8; 4096];
        let _ = socket.read(&mut buffer).await;
        let reply = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:{target_port}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        socket.write_all(reply.as_bytes()).await.unwrap();
    });
    let response = DefaultTransport
        .send(request(format!("http://127.0.0.1:{origin_port}/v1")))
        .await
        .unwrap();
    assert_eq!(response.status, 307);
    assert_eq!(check_status(response.status), Err(AiFailure::Redirected));
    let accepted =
        tokio::time::timeout(std::time::Duration::from_millis(200), target.accept()).await;
    assert!(accepted.is_err(), "запрос ушёл на адрес перенаправления");
}

/// Ссылка уводит чтение в запрещённую папку или служебное место — по строке пути
/// этого не видно, поэтому такой путь закрыт (ревью безопасности 050, M4).
#[cfg(unix)]
#[tokio::test]
async fn symlinks_are_closed_to_the_agent() {
    use std::os::unix::fs::symlink;
    use typsidian_core::ai::access::check_access_in;
    use typsidian_core::ai::settings::AiSettings;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("Личное")).unwrap();
    std::fs::create_dir_all(root.join("Публичное")).unwrap();
    std::fs::create_dir_all(root.join(".typsidian")).unwrap();
    std::fs::write(root.join("Личное/секрет.typ"), "секрет").unwrap();
    std::fs::write(root.join(".typsidian/ai-journal.jsonl"), "").unwrap();
    std::fs::write(root.join("Публичное/а.typ"), "а").unwrap();
    symlink(root.join("Личное"), root.join("Публичное/папка")).unwrap();
    symlink(
        root.join(".typsidian/ai-journal.jsonl"),
        root.join("Публичное/журнал.typ"),
    )
    .unwrap();
    let backend = NativeBackend::open(root).await.unwrap();
    let settings = AiSettings {
        enabled: true,
        denied_folders: vec!["Личное".to_owned()],
        ..AiSettings::default()
    };
    assert!(check_access_in(&backend, "Публичное/а.typ", &settings).await);
    assert!(!check_access_in(&backend, "Публичное/папка/секрет.typ", &settings).await);
    assert!(!check_access_in(&backend, "Публичное/журнал.typ", &settings).await);
}

// ── Фича 051: сессия урока ──

#[tokio::test]
async fn lesson_is_a_listed_kind_and_partial_stays_on_device() {
    lesson::lesson_is_a_listed_kind_and_partial_stays_on_device().await;
}

#[tokio::test]
async fn file_skips_broken_lines_and_appends_after_torn_line() {
    lesson::file_skips_broken_lines_and_appends_after_torn_line().await;
}

#[tokio::test]
async fn goal_name_and_folder_rules() {
    lesson::goal_name_and_folder_rules().await;
}

#[tokio::test]
async fn parser_assembles_anthropic_stream_from_arbitrary_chunks() {
    lesson::parser_assembles_anthropic_stream_from_arbitrary_chunks().await;
}

#[tokio::test]
async fn parser_assembles_compatible_stream_with_usage() {
    lesson::parser_assembles_compatible_stream_with_usage().await;
}

#[tokio::test]
async fn parser_does_not_take_a_cut_stream_for_a_reply() {
    lesson::parser_does_not_take_a_cut_stream_for_a_reply().await;
}

#[tokio::test]
async fn answer_is_on_disk_before_the_network() {
    lesson::answer_is_on_disk_before_the_network().await;
}

#[tokio::test]
async fn disabled_lesson_makes_no_requests() {
    lesson::disabled_lesson_makes_no_requests().await;
}

#[tokio::test]
async fn question_with_choices_and_open_question() {
    lesson::question_with_choices_and_open_question().await;
}

#[tokio::test]
async fn reply_without_tools_is_a_teacher_line() {
    lesson::reply_without_tools_is_a_teacher_line().await;
}

#[tokio::test]
async fn changed_lesson_is_refused_and_nothing_is_written() {
    lesson::changed_lesson_is_refused_and_nothing_is_written().await;
}

#[tokio::test]
async fn journal_failure_keeps_the_answer_and_stops_the_request() {
    lesson::journal_failure_keeps_the_answer_and_stops_the_request().await;
}

#[tokio::test]
async fn status_failure_is_a_refusal_without_server_text() {
    lesson::status_failure_is_a_refusal_without_server_text().await;
}

#[tokio::test]
async fn cancel_keeps_the_received_text_and_retry_needs_no_new_answer() {
    lesson::cancel_keeps_the_received_text_and_retry_needs_no_new_answer().await;
}

#[tokio::test]
async fn network_break_saves_what_arrived() {
    lesson::network_break_saves_what_arrived().await;
}

#[tokio::test]
async fn compatible_protocol_gets_stream_options() {
    lesson::compatible_protocol_gets_stream_options().await;
}

#[tokio::test]
async fn language_and_state_are_events() {
    lesson::language_and_state_are_events().await;
}

#[tokio::test]
async fn partial_file_restores_an_interrupted_reply() {
    lesson::partial_file_restores_an_interrupted_reply().await;
}

#[tokio::test]
async fn compaction_summarises_and_keeps_the_full_file() {
    lesson::compaction_summarises_and_keeps_the_full_file().await;
}

#[tokio::test]
async fn list_shows_all_lessons_with_states() {
    lesson::list_shows_all_lessons_with_states().await;
}

#[tokio::test]
async fn attached_note_goes_into_request_and_log() {
    lesson::attached_note_goes_into_request_and_log().await;
}

#[tokio::test]
async fn denied_missing_and_outside_notes_are_refused_without_text() {
    lesson::denied_missing_and_outside_notes_are_refused_without_text().await;
}

#[tokio::test]
async fn read_note_tool_continues_and_result_goes_next() {
    lesson::read_note_tool_continues_and_result_goes_next().await;
}

#[tokio::test]
async fn auto_continue_stops_after_four() {
    lesson::auto_continue_stops_after_four().await;
}

#[tokio::test]
async fn lesson_cost_equals_the_journal_sum() {
    lesson::lesson_cost_equals_the_journal_sum().await;
}

#[tokio::test]
async fn unknown_price_makes_the_lesson_cost_unknown() {
    lesson::unknown_price_makes_the_lesson_cost_unknown().await;
}

// ── Фича 051: потоковый транспорт на настоящем соединении ──

/// Одноразовый сервер: читает запрос, отдаёт заготовленные части с паузами.
async fn serve_once(head: &'static str, parts: Vec<Vec<u8>>) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = [0u8; 4096];
        let _ = socket.read(&mut buffer).await;
        if socket.write_all(head.as_bytes()).await.is_err() {
            return;
        }
        for part in parts {
            if socket.write_all(&part).await.is_err() {
                return;
            }
            let _ = socket.flush().await;
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        }
    });
    port
}

#[tokio::test]
async fn stream_arrives_in_parts_as_it_is_produced() {
    let port = serve_once(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        vec![b"data: 1\n\n".to_vec(), b"data: 2\n\n".to_vec()],
    )
    .await;
    let mut parts: Vec<Vec<u8>> = Vec::new();
    let status = DefaultTransport
        .send_stream(
            request(format!("http://127.0.0.1:{port}/v1")),
            &mut |chunk| parts.push(chunk.to_vec()),
        )
        .await
        .unwrap();
    assert_eq!(status, 200);
    assert!(parts.len() >= 2, "поток пришёл одним куском: {parts:?}");
    assert_eq!(parts.concat(), b"data: 1\n\ndata: 2\n\n");
}

#[tokio::test]
async fn stream_refusal_status_gives_no_body_to_the_core() {
    let port = serve_once(
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 6\r\nConnection: close\r\n\r\n",
        vec![b"SECRET".to_vec()],
    )
    .await;
    let mut fed = false;
    let status = DefaultTransport
        .send_stream(request(format!("http://127.0.0.1:{port}/v1")), &mut |_| {
            fed = true
        })
        .await
        .unwrap();
    assert_eq!(status, 500);
    assert!(!fed);
}

#[tokio::test]
async fn stream_over_the_limit_is_refused() {
    let port = serve_once(
        "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
        vec![vec![b'x'; 1024 * 1024]; 9],
    )
    .await;
    // Паузы между частями короткие, но предел общий для потока, а не для паузы.
    let mut total = 0usize;
    let failure = DefaultTransport
        .send_stream(
            request(format!("http://127.0.0.1:{port}/v1")),
            &mut |chunk| total += chunk.len(),
        )
        .await
        .unwrap_err();
    assert_eq!(failure, AiFailure::BadResponse);
    assert!(total <= typsidian_core::ai::transport::MAX_STREAM_BYTES);
}

/// Адрес внутренней сети остаётся закрытым и для потока: проверки сервера общие.
#[tokio::test]
async fn stream_keeps_the_redirect_rule() {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target.local_addr().unwrap().port();
    let head: &'static str = Box::leak(
        format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:{target_port}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_boxed_str(),
    );
    let port = serve_once(head, vec![]).await;
    let status = DefaultTransport
        .send_stream(request(format!("http://127.0.0.1:{port}/v1")), &mut |_| {})
        .await
        .unwrap();
    assert_eq!(check_status(status), Err(AiFailure::Redirected));
    let accepted =
        tokio::time::timeout(std::time::Duration::from_millis(200), target.accept()).await;
    assert!(accepted.is_err());
}

#[tokio::test]
async fn changed_connection_never_gets_the_old_key() {
    lesson::changed_connection_never_gets_the_old_key().await;
}

#[tokio::test]
async fn lesson_writes_never_go_through_symlinks() {
    lesson::lesson_writes_never_go_through_symlinks().await;
}

#[tokio::test]
async fn lesson_settings_are_sanitized_on_read() {
    lesson::lesson_settings_are_sanitized_on_read().await;
}

#[tokio::test]
async fn only_small_notes_are_read_and_lesson_files_have_a_limit() {
    lesson::only_small_notes_are_read_and_lesson_files_have_a_limit().await;
}

#[tokio::test]
async fn attachments_are_limited_normalized_and_initial_only() {
    lesson::attachments_are_limited_normalized_and_initial_only().await;
}

#[tokio::test]
async fn stream_parser_is_linear_and_refuses_a_huge_line() {
    lesson::stream_parser_is_linear_and_refuses_a_huge_line().await;
}

#[tokio::test]
async fn progress_buffer_is_delta_guarded_and_swept() {
    lesson::progress_buffer_is_delta_guarded_and_swept().await;
}

#[tokio::test]
async fn dropped_request_frees_its_slot_and_a_duplicate_keeps_the_buffer() {
    lesson::dropped_request_frees_its_slot_and_a_duplicate_keeps_the_buffer().await;
}

#[tokio::test]
async fn cancel_between_turns_is_remembered_for_the_whole_cycle() {
    lesson::cancel_between_turns_is_remembered_for_the_whole_cycle().await;
}

#[tokio::test]
async fn checkpoint_takes_the_path_from_the_turn_buffer() {
    lesson::checkpoint_takes_the_path_from_the_turn_buffer().await;
}

#[tokio::test]
async fn lesson_commands_accept_only_lesson_files() {
    lesson::lesson_commands_accept_only_lesson_files().await;
}

#[tokio::test]
async fn goal_name_avoids_reserved_names_and_format_characters() {
    lesson::goal_name_avoids_reserved_names_and_format_characters().await;
}

#[tokio::test]
async fn question_is_limited_in_choices_and_length() {
    lesson::question_is_limited_in_choices_and_length().await;
}

#[tokio::test]
async fn versions_folder_is_closed_to_the_agent() {
    lesson::versions_folder_is_closed_to_the_agent().await;
}

#[tokio::test]
async fn stignore_report_includes_the_lesson_partial_folder() {
    lesson::stignore_report_includes_the_lesson_partial_folder().await;
}

#[tokio::test]
async fn summary_tells_what_the_lesson_waits_for() {
    lesson::summary_tells_what_the_lesson_waits_for().await;
}

#[tokio::test]
async fn probe_refuses_a_connection_that_changed_after_the_check() {
    cases::probe_refuses_a_connection_that_changed_after_the_check().await;
}

/// M1, L5 на настоящей файловой системе: дописывание урока не идёт сквозь ссылку и не
/// оставляет временных файлов; запись урока не разбирается как Typst для таблицы ссылок.
#[cfg(unix)]
#[tokio::test]
async fn lesson_append_is_atomic_and_never_follows_a_symlink() {
    use std::os::unix::fs::symlink;
    use typsidian_core::ai::lesson::{
        create_lesson, open_lesson, set_state, Language, LessonState,
    };
    use typsidian_core::ai::settings::{save_settings, AiSettings};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    let outside = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(root.join("Уроки")).unwrap();
    std::fs::create_dir_all(root.join("Внешняя")).unwrap();
    std::fs::write(outside.path().join("чужое.lesson"), "ЧУЖОЙ-ФАЙЛ").unwrap();
    symlink(
        outside.path().join("чужое.lesson"),
        root.join("Уроки/ссылка.lesson"),
    )
    .unwrap();
    symlink(outside.path(), root.join("Внешняя/папка")).unwrap();

    let mut backend = NativeBackend::open(root).await.unwrap();
    let config = AiSettings {
        enabled: true,
        model: Some("m".to_owned()),
        ..AiSettings::default()
    };
    save_settings(&mut backend, &config).await.unwrap();

    // Ссылка вместо файла урока: отказ, внешний файл цел.
    assert!(set_state(
        &mut backend,
        "Уроки/ссылка.lesson",
        LessonState::Finished,
        1
    )
    .await
    .is_err());
    assert_eq!(
        std::fs::read_to_string(outside.path().join("чужое.lesson")).unwrap(),
        "ЧУЖОЙ-ФАЙЛ"
    );

    // Обычный урок: цель со ссылкой вида `[[…]]`, дописывание, ни одного временного файла.
    let path = create_lesson(&mut backend, "[[Нет такой]] цель", &[], Language::Ru, 1)
        .await
        .unwrap();
    set_state(&mut backend, &path, LessonState::Stopped, 2)
        .await
        .unwrap();
    let view = open_lesson(&mut backend, &path).await.unwrap();
    assert_eq!(view.state, LessonState::Stopped);
    assert!(backend
        .broken_links(std::path::Path::new(&path))
        .await
        .is_empty());
    let leftovers: Vec<_> = std::fs::read_dir(root.join(".typsidian/tmp"))
        .map(|entries| entries.flatten().collect())
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    // Папка уроков через ссылку наружу: урок не создаётся ни там, ни здесь.
    let mut linked = config.clone();
    linked.lessons_folder = "Внешняя/папка".to_owned();
    save_settings(&mut backend, &linked).await.unwrap();
    assert!(create_lesson(&mut backend, "Цель", &[], Language::Ru, 3)
        .await
        .is_err());
    assert!(std::fs::read_dir(outside.path())
        .unwrap()
        .flatten()
        .all(|entry| entry.file_name() == "чужое.lesson"));
}

/// M5: общий предел времени потока действует, даже если тишины между частями нет.
#[tokio::test]
async fn stream_over_the_total_time_is_cut() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = [0u8; 4096];
        let _ = socket.read(&mut buffer).await;
        let _ = socket
            .write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n")
            .await;
        // Часть приходит каждые 50 мс: пауза никогда не бывает долгой.
        loop {
            if socket.write_all(b"data: 1\n\n").await.is_err() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    });
    let mut received = 0usize;
    let failure = DefaultTransport
        .send_stream_within(
            request(format!("http://127.0.0.1:{port}/v1")),
            &mut |chunk| received += chunk.len(),
            std::time::Duration::from_millis(400),
        )
        .await
        .unwrap_err();
    assert_eq!(failure, AiFailure::Timeout);
    assert!(received > 0);
}
