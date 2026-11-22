// src/routes/frontend/edit.rs

use crate::auth::require_login;
use crate::database::DatabaseBackend;
use crate::domain::UpdatePhoto;
use crate::routes::sse::datastar_sse;
use crate::template::TemplateRenderer;
use crate::utils::{compact_html, e400};
use actix_identity::Identity;
use actix_web::Responder;
use actix_web::web::{Data, Path};
use datastar::actix_web::ReadSignals;
use datastar::prelude::PatchElements;
use std::sync::Arc;
use uuid::Uuid;

fn caption_context(id: Uuid, band: &str, tour: &str, venue: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "band": band,
        "tour": tour,
        "venue": venue,
        "signals": serde_json::json!({
            "band": band,
            "tour": tour,
            "venue": venue,
        }).to_string(),
    })
}

/// GET /gallery/{id}/edit
pub async fn edit_photo(
    path: Path<String>,
    database: Data<Arc<dyn DatabaseBackend>>,
    templates: Data<dyn TemplateRenderer>,
    identity: Option<Identity>,
) -> Result<impl Responder, actix_web::Error> {
    require_login(&identity)?;
    let id = Uuid::parse_str(&path.into_inner()).map_err(e400)?;
    let photo = database.read_photo(id).await?;

    let context = caption_context(id, &photo.band, &photo.tour, &photo.venue);
    let html = compact_html(&templates.render("partials/edit_form.html", &context)?);

    Ok(datastar_sse([PatchElements::new(html).into_datastar_event()]))
}

/// PUT /gallery/{id}
pub async fn update_photo(
    path: Path<String>,
    ReadSignals(updates): ReadSignals<UpdatePhoto>,
    database: Data<Arc<dyn DatabaseBackend>>,
    templates: Data<dyn TemplateRenderer>,
    identity: Option<Identity>,
) -> Result<impl Responder, actix_web::Error> {
    require_login(&identity)?;
    let id = Uuid::parse_str(&path.into_inner()).map_err(e400)?;

    database.update_photo(id, updates).await?;
    let photo = database.read_photo(id).await?;

    let context = serde_json::json!({
        "id": id,
        "band": photo.band,
        "tour": photo.tour,
        "venue": photo.venue,
    });
    let html = compact_html(&templates.render("partials/caption.html", &context)?);

    Ok(datastar_sse([PatchElements::new(html).into_datastar_event()]))
}

/// GET /gallery/{id}/cancel
pub async fn cancel_edit(
    path: Path<String>,
    database: Data<Arc<dyn DatabaseBackend>>,
    templates: Data<dyn TemplateRenderer>,
    identity: Option<Identity>,
) -> Result<impl Responder, actix_web::Error> {
    require_login(&identity)?;
    let id = Uuid::parse_str(&path.into_inner()).map_err(e400)?;
    let photo = database.read_photo(id).await?;

    let context = serde_json::json!({
        "id": id,
        "band": photo.band,
        "tour": photo.tour,
        "venue": photo.venue,
    });
    let html = compact_html(&templates.render("partials/caption.html", &context)?);

    Ok(datastar_sse([PatchElements::new(html).into_datastar_event()]))
}
