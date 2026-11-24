// tests/api/photos.rs

use crate::helpers::spawn_app;
use reqwest::StatusCode;
use reqwest::multipart::{Form, Part};
use uuid::Uuid;

fn heic_fixture() -> &'static [u8] {
    include_bytes!("../fixtures/IMG_2215.HEIC")
}

#[tokio::test]
async fn login_with_bad_credentials_redirects_to_login_error_and_logout_clears_session() {
    let app = spawn_app().await;

    let bad = app
        .unauthenticated_client
        .post(format!("{}/login", app.address))
        .form(&[("username", "wrong"), ("password", "wrong")])
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        bad.headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/login?error=1")
    );

    // Authenticated client can access `/`, then log out and lose access.
    assert_eq!(app.get("/").await.status(), StatusCode::OK);

    let logout = app.get("/logout").await;
    assert_eq!(logout.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        logout
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/galleries")
    );

    let after = app.get("/").await;
    assert_eq!(after.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        after
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/login")
    );
}

#[tokio::test]
async fn photo_upload_public_gallery_inline_edit_and_delete_round_trip() {
    let app = spawn_app().await;

    // 1. Upload via API with HEIC fixture.
    let form = Form::new()
        .text("band", "Iron Maiden")
        .text("tour", "Future Past Tour")
        .text("venue", "Rogers Arena")
        .part(
            "file",
            Part::bytes(heic_fixture().to_vec()).file_name("IMG_2215.HEIC"),
        );

    let create = app
        .api_client
        .post(format!("{}/api/v1/photos", app.address))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let photo_id: Uuid = create.json().await.unwrap();

    // 2. Public image and gallery endpoints serve it without auth.
    let img = app
        .unauthenticated_client
        .get(format!("{}/api/v1/photos/{photo_id}/image", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(img.status(), StatusCode::OK);
    assert_eq!(
        img.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("image/jpeg")
    );
    let bytes = img.bytes().await.unwrap();
    assert!(bytes.starts_with(&[0xFF, 0xD8]));

    let galleries_html = app
        .unauthenticated_client
        .get(format!("{}/galleries", app.address))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(galleries_html.contains("Iron Maiden"));
    assert!(galleries_html.contains("/g/iron-maiden"));

    let band_html = app
        .unauthenticated_client
        .get(format!("{}/g/iron-maiden", app.address))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(band_html.contains("Future Past Tour"));
    assert!(band_html.contains("Rogers Arena"));
    assert!(band_html.contains(&format!("/api/v1/photos/{photo_id}/image")));
    assert!(band_html.contains("photo-thumb-button"));
    assert!(band_html.contains("class=\"lightbox\""));

    // 3. Inline edit via Datastar SSE.
    let edit_form = app
        .get(&format!("/gallery/{photo_id}/edit"))
        .await
        .text()
        .await
        .unwrap();
    assert!(edit_form.contains(&format!("/gallery/{photo_id}")));

    let updated = app
        .api_client
        .put(format!("{}/gallery/{photo_id}", app.address))
        .json(&serde_json::json!({
            "band": "Iron Maiden",
            "tour": "Run For Your Lives",
            "venue": "Pacific Coliseum"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated_body = updated.text().await.unwrap();
    assert!(updated_body.contains("Run For Your Lives"));
    assert!(updated_body.contains("Pacific Coliseum"));

    // 4. Delete via browser route removes photo from DB & storage.
    let del = app
        .api_client
        .delete(format!("{}/gallery/{photo_id}", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::OK);
    let del_body = del.text().await.unwrap();
    assert!(del_body.contains(&format!("#photo-{photo_id}")));

    let missing = app
        .unauthenticated_client
        .get(format!("{}/api/v1/photos/{photo_id}", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn jpeg_photo_upload_succeeds_and_invalid_image_returns_400() {
    let app = spawn_app().await;

    let jpeg_bytes = server::conversion::convert_heic_to_jpeg(heic_fixture()).unwrap();

    // 1. Upload a valid JPEG.
    let form = Form::new()
        .text("band", "Judas Priest")
        .text("tour", "Invincible Shield Tour")
        .text("venue", "Pacific Coliseum")
        .part(
            "file",
            Part::bytes(jpeg_bytes.clone()).file_name("show.jpg"),
        );

    let create = app
        .api_client
        .post(format!("{}/api/v1/photos", app.address))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let photo_id: Uuid = create.json().await.unwrap();

    let img = app
        .unauthenticated_client
        .get(format!("{}/api/v1/photos/{photo_id}/image", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(img.status(), StatusCode::OK);
    assert_eq!(img.bytes().await.unwrap().as_ref(), jpeg_bytes.as_slice());

    // 2. Upload invalid bytes -> 400 Bad Request.
    let bad_form = Form::new()
        .text("band", "Judas Priest")
        .text("tour", "Invincible Shield Tour")
        .text("venue", "Pacific Coliseum")
        .part(
            "file",
            Part::bytes(b"not a real image".to_vec()).file_name("broken.jpg"),
        );

    let bad = app
        .api_client
        .post(format!("{}/api/v1/photos", app.address))
        .multipart(bad_form)
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn browser_upload_error_renders_styled_error_page() {
    let app = spawn_app().await;

    let bad_form = Form::new()
        .text("band", "Judas Priest")
        .text("tour", "Invincible Shield Tour")
        .text("venue", "Pacific Coliseum")
        .part(
            "file",
            Part::bytes(b"definitely not an image".to_vec()).file_name("notes.txt"),
        );

    let response = app
        .api_client
        .post(format!("{}/upload", app.address))
        .multipart(bad_form)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.starts_with("text/html"),
        "Expected text/html, got {content_type}"
    );

    let body = response.text().await.unwrap();
    assert!(body.contains("<!DOCTYPE html>"));
    assert!(body.contains("error-page"));
    assert!(body.contains("400"));
    assert!(body.contains("Upload Failed"));
    assert!(body.contains("notes.txt"));
    assert!(body.contains(r#"href="/upload""#));
}
