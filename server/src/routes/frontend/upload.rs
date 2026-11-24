// src/routes/frontend/upload.rs

use crate::auth::require_login;
use crate::database::DatabaseBackend;
use crate::services::PhotoOperations;
use crate::storage::StorageBackend;
use crate::template::TemplateRenderer;
use actix_identity::Identity;
use actix_multipart::form::{MultipartForm, tempfile::TempFile, text::Text};
use actix_web::HttpResponse;
use actix_web::http::StatusCode;
use actix_web::web::{self, Data};
use serde::Serialize;
use std::fs;
use std::sync::Arc;

#[derive(Serialize)]
struct PageContext<'a> {
    title: &'a str,
    header: &'a str,
    sub_header: &'a str,
    logged_in: bool,
}

#[derive(Serialize)]
struct UploadErrorContext<'a> {
    title: &'a str,
    status_code: String,
    heading: &'a str,
    message: String,
    back_url: &'a str,
    back_label: &'a str,
    logged_in: bool,
}

fn format_upload_error(file_name: Option<&str>, detail: &str) -> String {
    match file_name.filter(|name| !name.is_empty()) {
        Some(name) => format!("Could not upload \"{name}\": {detail}"),
        None => detail.to_string(),
    }
}

fn render_upload_error(
    templates: &dyn TemplateRenderer,
    status: StatusCode,
    message: String,
) -> Result<HttpResponse, actix_web::Error> {
    let context = UploadErrorContext {
        title: "Upload Error",
        status_code: status.as_u16().to_string(),
        heading: "Upload Failed",
        message,
        back_url: "/upload",
        back_label: "Back to Upload",
        logged_in: true,
    };
    let json_context = serde_json::to_value(&context)?;
    let html = templates.render("error.html", &json_context)?;
    Ok(HttpResponse::build(status)
        .content_type("text/html; charset=utf-8")
        .body(html))
}

#[derive(Debug, MultipartForm)]
pub struct UploadForm {
    band: Text<String>,
    tour: Text<String>,
    venue: Text<String>,
    #[multipart(rename = "file")]
    photo_file: Vec<TempFile>,
}

/// GET /upload — render the upload form.
pub async fn get_upload_page(
    templates: Data<dyn TemplateRenderer>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    require_login(&identity)?;
    let context = PageContext {
        title: "Upload",
        header: "Upload Photos",
        sub_header: "Concert Photo Galleries",
        logged_in: true,
    };

    let json_context = serde_json::to_value(&context)?;
    let html = templates.render("upload.html", &json_context)?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

/// POST /upload — browser-facing upload.
pub async fn post_upload(
    MultipartForm(form): MultipartForm<UploadForm>,
    templates: Data<dyn TemplateRenderer>,
    database: Data<Arc<dyn DatabaseBackend>>,
    storage: Data<Arc<dyn StorageBackend>>,
    operations: Data<PhotoOperations>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    require_login(&identity)?;

    if form.photo_file.is_empty() {
        return render_upload_error(
            templates.as_ref(),
            StatusCode::BAD_REQUEST,
            "Please select at least one photo to upload.".to_string(),
        );
    }

    let band = form.band.into_inner();
    let tour = form.tour.into_inner();
    let venue = form.venue.into_inner();

    for photo_file in form.photo_file {
        let file_name = photo_file.file_name.clone();
        let photo_file_bytes = match web::block(move || fs::read(photo_file.file.path())).await {
            Ok(Ok(bytes)) => bytes.into(),
            Ok(Err(err)) => {
                tracing::warn!(error = ?err, file_name = ?file_name, "failed to read uploaded file");
                return render_upload_error(
                    templates.as_ref(),
                    StatusCode::BAD_REQUEST,
                    format_upload_error(
                        file_name.as_deref(),
                        "Could not read the uploaded file.",
                    ),
                );
            }
            Err(err) => {
                tracing::error!(error = ?err, file_name = ?file_name, "upload file read task failed");
                return render_upload_error(
                    templates.as_ref(),
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format_upload_error(
                        file_name.as_deref(),
                        "A server error occurred while reading the uploaded file.",
                    ),
                );
            }
        };

        if let Err(err) = operations
            .create_photo_from_bytes(
                photo_file_bytes,
                band.clone(),
                tour.clone(),
                venue.clone(),
                database.as_ref().as_ref(),
                storage.as_ref().as_ref(),
            )
            .await
        {
            let status = err.as_response_error().status_code();
            let message = if status.is_client_error() {
                format_upload_error(file_name.as_deref(), &err.to_string())
            } else {
                tracing::error!(error = ?err, file_name = ?file_name, "photo upload failed");
                format_upload_error(
                    file_name.as_deref(),
                    "A server error occurred while saving the photo. Please try again.",
                )
            };
            return render_upload_error(templates.as_ref(), status, message);
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/gallery?status=success"))
        .finish())
}
