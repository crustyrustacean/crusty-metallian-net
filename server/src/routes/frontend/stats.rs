// src/routes/frontend/stats.rs

use crate::database::{DatabaseBackend, DatabaseError};
use crate::domain::RotationEntry;
use crate::template::{TemplateError, TemplateRenderer};
use crate::utils::error_chain_fmt;
use actix_web::{HttpResponse, ResponseError, web};
use serde::Serialize;
use std::sync::Arc;

#[derive(thiserror::Error)]
pub enum StatsPageError {
    #[error("database operation failed")]
    Database(#[from] DatabaseError),
    #[error("template rendering failed")]
    Template(#[from] TemplateError),
    #[error("context serialization failed")]
    Serialize(#[from] serde_json::Error),
}

impl std::fmt::Debug for StatsPageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for StatsPageError {
    fn error_response(&self) -> HttpResponse {
        tracing::error!(error = ?self, "stats page failed");
        HttpResponse::InternalServerError().body("Internal Server Error")
    }
}

#[derive(Serialize)]
struct StatsPageContext<'a> {
    title: &'a str,
    header: &'a str,
    sub_header: &'a str,
    rotation_entries: Vec<RotationEntry>,
}

pub async fn get_stats_page(
    templates: web::Data<dyn TemplateRenderer>,
    database: web::Data<Arc<dyn DatabaseBackend>>,
) -> Result<HttpResponse, StatsPageError> {
    let rotation_entries = database.list().await?;
    let context = StatsPageContext {
        title: "Stats",
        header: "Stats & Rotation",
        sub_header: "Keep track of what's been on rotation.",
        rotation_entries,
    };

    let json_ctx = serde_json::to_value(&context)?;
    let html = templates.render("stats.html", &json_ctx)?;

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}
