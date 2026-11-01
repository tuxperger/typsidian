//! wasm-bindgen-test — только wasm32 (принцип II). Парный — `ai_native.rs`.
#![cfg(target_arch = "wasm32")]

mod common;

use common::ai_cases as cases;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn anthropic_gets_a_default_model() {
    cases::anthropic_gets_a_default_model().await;
}

#[wasm_bindgen_test]
async fn missing_or_broken_settings_mean_disabled() {
    cases::missing_or_broken_settings_mean_disabled().await;
}

#[wasm_bindgen_test]
async fn disabled_makes_no_requests() {
    cases::disabled_makes_no_requests().await;
}

#[wasm_bindgen_test]
async fn journal_gets_request_before_sending_and_never_the_key() {
    cases::journal_gets_request_before_sending_and_never_the_key().await;
}

#[wasm_bindgen_test]
async fn journal_failure_stops_the_request() {
    cases::journal_failure_stops_the_request().await;
}

#[wasm_bindgen_test]
async fn unknown_price_is_not_zero() {
    cases::unknown_price_is_not_zero().await;
}

#[wasm_bindgen_test]
async fn failure_is_recorded_and_carries_no_server_text() {
    cases::failure_is_recorded_and_carries_no_server_text().await;
}

#[wasm_bindgen_test]
async fn statuses_map_to_failures() {
    cases::statuses_map_to_failures().await;
}

#[wasm_bindgen_test]
async fn journal_keeps_last_entries_and_clears() {
    cases::journal_keeps_last_entries_and_clears().await;
}

#[wasm_bindgen_test]
async fn journal_stays_out_of_sync_and_settings_travel() {
    cases::journal_stays_out_of_sync_and_settings_travel().await;
}

#[wasm_bindgen_test]
async fn endpoints_are_guarded() {
    cases::endpoints_are_guarded().await;
}

#[wasm_bindgen_test]
async fn cloud_key_never_goes_to_custom_endpoint() {
    cases::cloud_key_never_goes_to_custom_endpoint().await;
}

#[wasm_bindgen_test]
async fn models_are_listed_sorted() {
    cases::models_are_listed_sorted().await;
}

#[wasm_bindgen_test]
async fn access_stays_inside_the_vault() {
    cases::access_stays_inside_the_vault().await;
}

#[wasm_bindgen_test]
async fn denied_folder_follows_the_move() {
    cases::denied_folder_follows_the_move().await;
}

#[wasm_bindgen_test]
async fn cancel_stops_waiting_at_once() {
    cases::cancel_stops_waiting_at_once().await;
}

#[wasm_bindgen_test]
async fn access_ignores_unicode_form() {
    cases::access_ignores_unicode_form().await;
}

#[wasm_bindgen_test]
async fn duplicate_request_id_keeps_the_first_cancellable() {
    cases::duplicate_request_id_keeps_the_first_cancellable().await;
}

#[wasm_bindgen_test]
async fn custom_endpoint_needs_approval_on_this_device() {
    cases::custom_endpoint_needs_approval_on_this_device().await;
}
