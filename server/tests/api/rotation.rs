// tests/api/rotation.rs

use crate::helpers::{
    assert_sse_framing, assert_sse_headers, spawn_app, spawn_app_against, sse_body,
};
use crate::metadata_stub::MetadataStubs;
use reqwest::StatusCode;

fn payload() -> serde_json::Value {
    serde_json::json!({
        "artist": "Sabaton",
        "album": "Attero Dominatus",
        "note": "A co-worker turned me on to Sabaton in early 2022."
    })
}

#[tokio::test]
async fn post_rotation_entry_returns_append_patch_and_clears_signals() {
    let app = spawn_app().await;

    let response = app
        .api_client
        .post(format!("{}/api/v1/rotation", app.address))
        .json(&payload())
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_sse_headers(&response);
    let body = sse_body(response).await;
    let events = assert_sse_framing(&body, 3);

    assert_eq!(events[0].dataline("selector"), Some("#rotation-list"));
    assert_eq!(events[0].dataline("mode"), Some("append"));
    assert!(body.contains(r#"class="rotation-artist""#));
    assert!(body.contains("Sabaton"));
    assert!(body.contains("Attero Dominatus"));
    assert_eq!(events[1].dataline("selector"), Some("#rotation-loading"));
    assert_eq!(events[1].dataline("mode"), Some("remove"));
    assert_eq!(events[2].event.as_deref(), Some("datastar-patch-signals"));
    assert!(body.contains(r#""artist":"""#));
}

#[tokio::test]
async fn post_rotation_entry_with_table_view_targets_table_body() {
    let app = spawn_app().await;

    let response = app
        .api_client
        .post(format!("{}/api/v1/rotation?view=table", app.address))
        .json(&payload())
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    let body = sse_body(response).await;
    let events = assert_sse_framing(&body, 3);
    assert_eq!(events[0].dataline("selector"), Some("#rotation-table-body"));
    assert!(body.contains(r#"id="rotation-entry-1""#));
}

#[tokio::test]
async fn posted_entry_appears_on_stats_page_with_metadata() {
    let mb_id = "b4974b0b-305e-4f98-b16d-04112f3cced2";
    let stubs = MetadataStubs::full_hit(mb_id).await;
    let app = spawn_app_against(stubs).await;

    let response = app
        .api_client
        .post(format!("{}/api/v1/rotation?view=table", app.address))
        .json(&payload())
        .send()
        .await
        .expect("Failed to execute request.");
    assert!(response.status().is_success());

    let page = app.get("/stats").await.text().await.unwrap();
    assert!(page.contains("Sabaton"));
    assert!(page.contains("Attero Dominatus"));
    assert!(page.contains("2006"));
    assert!(page.contains("front-250.jpg"));
}

#[tokio::test]
async fn failing_metadata_service_still_saves_entry_and_reports_status() {
    let stubs = MetadataStubs::failure().await;
    let app = spawn_app_against(stubs).await;

    let response = app
        .api_client
        .post(format!("{}/api/v1/rotation", app.address))
        .json(&payload())
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    let body = sse_body(response).await;
    assert!(body.contains("Could not look that up"));

    let page = app.get("/stats").await.text().await.unwrap();
    assert!(page.contains("Sabaton"));
}

#[tokio::test]
async fn get_rotation_returns_404_when_empty_and_200_when_populated() {
    let app = spawn_app().await;

    let empty = app.get("/api/v1/rotation").await;
    assert_eq!(empty.status(), StatusCode::NOT_FOUND);

    let post = app
        .api_client
        .post(format!("{}/api/v1/rotation", app.address))
        .json(&payload())
        .send()
        .await
        .unwrap();
    assert!(post.status().is_success());

    let filled = app.get("/api/v1/rotation").await;
    assert_eq!(filled.status(), StatusCode::OK);
    assert_sse_headers(&filled);
    let body = sse_body(filled).await;
    let events = assert_sse_framing(&body, 2);
    assert_eq!(events[0].dataline("selector"), Some("#rotation-list"));
    assert_eq!(events[1].dataline("selector"), Some("#rotation-loading"));
    assert_eq!(events[1].dataline("mode"), Some("remove"));
}

#[tokio::test]
async fn release_without_art_saves_year_without_cover_image() {
    let stubs = MetadataStubs::start().await;
    stubs.expect_no_art("bootleg-0001").await;
    let app = spawn_app_against(stubs).await;

    let response = app
        .api_client
        .post(format!("{}/api/v1/rotation?view=table", app.address))
        .json(&payload())
        .send()
        .await
        .expect("Failed to execute request.");
    assert!(response.status().is_success());

    let page = app.get("/stats").await.text().await.unwrap();
    assert!(page.contains("2006"));
    assert!(page.contains("No cover"));
    assert!(!page.contains("front-250.jpg"));
}

#[tokio::test]
async fn admin_pages_and_post_require_basic_auth_while_public_gets_stay_open() {
    let app = spawn_app().await;

    for path in ["/", "/stats"] {
        let resp = app
            .unauthenticated_client
            .get(format!("{}{path}", app.address))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "GET {path}");
        assert!(resp.headers().contains_key(reqwest::header::WWW_AUTHENTICATE));
    }

    let post_resp = app
        .unauthenticated_client
        .post(format!("{}/api/v1/rotation", app.address))
        .json(&payload())
        .send()
        .await
        .unwrap();
    assert_eq!(post_resp.status(), StatusCode::UNAUTHORIZED);
    assert!(
        post_resp
            .headers()
            .contains_key(reqwest::header::WWW_AUTHENTICATE)
    );

    // Public endpoints stay open without credentials.
    let health = app
        .unauthenticated_client
        .get(format!("{}/api/v1/health_check", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    let stats = app
        .unauthenticated_client
        .get(format!("{}/api/v1/stats", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(stats.status(), StatusCode::OK);

    let rotation = app
        .unauthenticated_client
        .get(format!("{}/api/v1/rotation", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(rotation.status(), StatusCode::NOT_FOUND);
}
