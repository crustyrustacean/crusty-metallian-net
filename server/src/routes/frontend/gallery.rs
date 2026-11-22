// src/routes/frontend/gallery.rs

use crate::auth::require_login;
use crate::database::DatabaseBackend;
use crate::domain::Photo;
use crate::template::TemplateRenderer;
use actix_identity::Identity;
use actix_web::{HttpResponse, web::Data};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct PageContext<'a> {
    title: &'a str,
    header: &'a str,
    sub_header: &'a str,
    logged_in: bool,
    photos: Vec<Photo>,
}

pub async fn get_gallery_page(
    templates: Data<dyn TemplateRenderer>,
    database: Data<Arc<dyn DatabaseBackend>>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    require_login(&identity)?;
    let photos = database.list_photos().await?;

    let context = PageContext {
        title: "Manage Gallery",
        header: "Manage Gallery",
        sub_header: "Concert Photo Galleries",
        logged_in: true,
        photos,
    };

    let json_context = serde_json::to_value(&context)?;
    let html = templates.render("gallery.html", &json_context)?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}
