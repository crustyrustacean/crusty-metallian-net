// tests/api/stats.rs

use crate::helpers::{assert_sse_framing, assert_sse_headers, spawn_app, sse_body};

#[tokio::test]
async fn get_artist_stats_appends_widget_and_clears_placeholder() {
    let app = spawn_app().await;
    let seed = app
        .api_client
        .post(format!("{}/api/v1/rotation", app.address))
        .json(&serde_json::json!({
            "artist": "Sabaton",
            "album": "Attero Dominatus",
            "note": ""
        }))
        .send()
        .await
        .expect("Failed to execute request.");
    assert!(seed.status().is_success());
    sse_body(seed).await;

    let response = app.get("/api/v1/stats").await;
    assert!(response.status().is_success());
    assert_sse_headers(&response);

    let body = sse_body(response).await;
    let events = assert_sse_framing(&body, 2);
    assert_eq!(events[0].dataline("selector"), Some("#artist-stats-list"));
    assert_eq!(events[0].dataline("mode"), Some("append"));
    assert!(body.contains("artist-stats-widget"));
    assert!(body.contains("Sabaton"));
    assert!(body.contains(r#"class="artist-stats-spins">1<"#));
    assert_eq!(
        events[1].dataline("selector"),
        Some("#artist-stats-loading")
    );
    assert_eq!(events[1].dataline("mode"), Some("remove"));
}

#[tokio::test]
async fn get_artist_stats_renders_empty_state_without_entries() {
    let app = spawn_app().await;

    let response = app.get("/api/v1/stats").await;
    assert!(response.status().is_success());
    let body = sse_body(response).await;
    assert!(body.contains("No spins recorded yet."));
}

#[tokio::test]
async fn stats_control_panel_page_renders_form_and_empty_table() {
    let app = spawn_app().await;

    let response = app.get("/stats").await;
    assert_eq!(reqwest::StatusCode::OK, response.status());
    let body = response.text().await.unwrap();
    assert!(body.contains(r#"id="rotation-form""#));
    assert!(body.contains("@post('/api/v1/rotation?view=table')"));
    assert!(body.contains("Nothing spinning yet."));
}
