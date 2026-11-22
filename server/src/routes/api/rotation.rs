// src/routes/api/rotation.rs

use crate::auth::require_api_login;
use crate::database::{DatabaseBackend, DatabaseError};
use crate::domain::{NewRotationEntry, RotationEntry};
use crate::metadata::{AlbumQuery, MetadataClient};
use crate::routes::sse::datastar_sse;
use crate::template::{TemplateError, TemplateRenderer};
use crate::utils::error_chain_fmt;
use actix_identity::Identity;
use actix_web::{HttpResponse, Responder, ResponseError, http::StatusCode, web};
use datastar::actix_web::ReadSignals;
use datastar::prelude::*;
use jiff::ZonedDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(thiserror::Error)]
pub enum RotationEntryError {
    #[error("rotation entry not found")]
    NotFound,
    #[error("database operation failed")]
    Database(#[from] DatabaseError),
    #[error("template rendering failed")]
    Template(#[from] TemplateError),
    #[error("context serialization failed")]
    Serialize(#[from] serde_json::Error),
}

impl std::fmt::Debug for RotationEntryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for RotationEntryError {
    fn status_code(&self) -> StatusCode {
        match self {
            RotationEntryError::NotFound => StatusCode::NOT_FOUND,
            RotationEntryError::Database(_)
            | RotationEntryError::Template(_)
            | RotationEntryError::Serialize(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        tracing::error!(error = ?self, "request failed");
        HttpResponse::build(self.status_code()).body("Something went wrong.")
    }
}

/// The element a consumer puts inside `#rotation-list` (or the table body) as a
/// placeholder, removed once a real entry is appended.
const PLACEHOLDER_ID: &str = "#rotation-loading";

/// What the form sends. `cover` and `year` are absent by design — they are looked
/// up, not typed. `id` and `listened_date` are absent because the app controls
/// them, so a client cannot forge either.
#[derive(Clone, Deserialize, Serialize)]
pub struct RawRotationEntry {
    artist: String,
    album: String,
    note: String,
}

/// Signal used to tell the user the lookup failed without discarding the entry.
const METADATA_STATUS: &str = "metadata_status";
const LOOKUP_FAILED: &str = "Could not look that up - saved without a cover or year";

#[derive(Serialize)]
struct RotationTemplateContext<'a> {
    rotation_entry: &'a RotationEntry,
}

/// Render a single entry as either the public widget or a control-panel table row.
fn render_rotation(
    templates: &dyn TemplateRenderer,
    entry: &RotationEntry,
    template: &str,
) -> Result<String, RotationEntryError> {
    let ctx = serde_json::to_value(RotationTemplateContext {
        rotation_entry: entry,
    })?;
    Ok(templates.render(template, &ctx)?)
}

#[derive(Deserialize)]
pub struct RotationView {
    view: Option<String>,
}

pub async fn post_rotation_entry_ds(
    templates: web::Data<dyn TemplateRenderer>,
    database: web::Data<Arc<dyn DatabaseBackend>>,
    metadata: web::Data<MetadataClient>,
    view: web::Query<RotationView>,
    ReadSignals(raw_rotation_entry): ReadSignals<RawRotationEntry>,
    identity: Option<Identity>,
) -> Result<impl Responder, actix_web::Error> {
    require_api_login(&identity)?;
    // Look the album up before anything else. This never blocks the save: a
    // miss and a failure both come back as empty metadata, and the user is told
    // which happened via the status signal below.
    let query = AlbumQuery::new(&raw_rotation_entry.artist, &raw_rotation_entry.album);
    let looked_up = metadata.lookup(&query).await;

    let lookup_failed = looked_up.is_err();
    let metadata = looked_up.unwrap_or_default();

    let new_entry = NewRotationEntry {
        listened_date: ZonedDateTime::now().date(),
        artist: raw_rotation_entry.artist,
        album: raw_rotation_entry.album,
        cover: metadata.cover,
        year: metadata.year,
        note: (!raw_rotation_entry.note.is_empty()).then_some(raw_rotation_entry.note),
    };
    let entry = database
        .insert(new_entry)
        .await
        .map_err(RotationEntryError::Database)?;

    // The form requests a table row; the default remains the public widget
    // response.
    let (template, selector) = if view.view.as_deref() == Some("table") {
        ("rotation_row.html", "#rotation-table-body")
    } else {
        ("rotation.html", "#rotation-list")
    };
    let rendered = render_rotation(templates.as_ref(), &entry, template)?;

    let patch = PatchElements::new(rendered)
        .selector(selector)
        .mode(ElementPatchMode::Append)
        .into_datastar_event();

    let clear_empty = PatchElements::new_remove(PLACEHOLDER_ID).into_datastar_event();

    let clear = PatchSignals::new(format!(
        r#"{{"artist":"","album":"","note":"","{METADATA_STATUS}":""}}"#
    ))
    .into_datastar_event();

    let mut events = vec![patch, clear_empty, clear];

    if lookup_failed {
        let status = PatchSignals::new(format!(r#"{{"{METADATA_STATUS}":"{LOOKUP_FAILED}"}}"#))
            .into_datastar_event();
        events.push(status);
    }

    Ok(datastar_sse(events))
}

pub async fn get_rotation_entry(
    templates: web::Data<dyn TemplateRenderer>,
    database: web::Data<Arc<dyn DatabaseBackend>>,
) -> Result<impl Responder, RotationEntryError> {
    let entry = database.random().await.map_err(|error| match error {
        DatabaseError::NotFound => RotationEntryError::NotFound,
        other => RotationEntryError::Database(other),
    })?;

    let rendered = render_rotation(templates.as_ref(), &entry, "rotation.html")?;

    let patch = PatchElements::new(rendered)
        .selector("#rotation-list")
        .mode(ElementPatchMode::Append)
        .into_datastar_event();
    let clear_placeholder = PatchElements::new_remove(PLACEHOLDER_ID).into_datastar_event();

    Ok(datastar_sse([patch, clear_placeholder]))
}
