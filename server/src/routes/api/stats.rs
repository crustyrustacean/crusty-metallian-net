// src/routes/api/stats.rs

use crate::database::{DatabaseBackend, DatabaseError};
use crate::domain::ArtistStat;
use crate::routes::sse::datastar_sse;
use crate::template::{TemplateError, TemplateRenderer};
use crate::utils::{compact_html, error_chain_fmt};
use actix_web::{HttpResponse, Responder, ResponseError, web};
use datastar::prelude::*;
use serde::Serialize;
use std::sync::Arc;

#[derive(thiserror::Error)]
pub enum ArtistStatError {
    #[error("database operation failed")]
    Database(#[from] DatabaseError),
    #[error("template rendering failed")]
    Template(#[from] TemplateError),
    #[error("context serialization failed")]
    Serialize(#[from] serde_json::Error),
}

impl std::fmt::Debug for ArtistStatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ArtistStatError {
    fn error_response(&self) -> HttpResponse {
        tracing::error!(error = ?self, "request failed");
        HttpResponse::InternalServerError().body("Something went wrong.")
    }
}

const PLACEHOLDER_ID: &str = "#artist-stats-loading";

/// How many artists the widget receives.
const ARTIST_STATS_LIMIT: i64 = 20;

#[derive(Serialize)]
struct ArtistStatsContext<'a> {
    artist_stats: &'a [ArtistStat],
}

fn render_artist_stats(
    templates: &dyn TemplateRenderer,
    artist_stats: &[ArtistStat],
) -> Result<String, ArtistStatError> {
    let ctx = serde_json::to_value(ArtistStatsContext { artist_stats })?;
    Ok(compact_html(&templates.render("artist_stats.html", &ctx)?))
}

pub async fn get_artist_stats(
    templates: web::Data<dyn TemplateRenderer>,
    database: web::Data<Arc<dyn DatabaseBackend>>,
) -> Result<impl Responder, ArtistStatError> {
    let artist_stats = database
        .artist_stats(ARTIST_STATS_LIMIT)
        .await
        .map_err(ArtistStatError::Database)?;

    let rendered = render_artist_stats(templates.as_ref(), &artist_stats)?;

    let patch = PatchElements::new(rendered)
        .selector("#artist-stats-list")
        .mode(ElementPatchMode::Append)
        .into_datastar_event();
    let clear_placeholder = PatchElements::new_remove(PLACEHOLDER_ID).into_datastar_event();

    Ok(datastar_sse([patch, clear_placeholder]))
}
