// src/routes/frontend/stats.rs

use crate::auth::require_login;
use crate::database::DatabaseBackend;
use crate::domain::RotationEntry;
use crate::template::TemplateRenderer;
use actix_identity::Identity;
use actix_web::{HttpResponse, web};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct StatsPageContext<'a> {
    title: &'a str,
    header: &'a str,
    sub_header: &'a str,
    logged_in: bool,
    rotation_entries: Vec<RotationEntry>,
}

pub async fn get_stats_page(
    templates: web::Data<dyn TemplateRenderer>,
    database: web::Data<Arc<dyn DatabaseBackend>>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    require_login(&identity)?;
    let rotation_entries = database.list().await?;
    let context = StatsPageContext {
        title: "Stats",
        header: "Stats & Rotation",
        sub_header: "Keep track of what's been on rotation.",
        logged_in: true,
        rotation_entries,
    };

    let json_ctx = serde_json::to_value(&context)?;
    let html = templates.render("stats.html", &json_ctx)?;

    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}
