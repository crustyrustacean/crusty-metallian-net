// src/routes/frontend/galleries.rs

use crate::auth::is_logged_in;
use crate::database::DatabaseBackend;
use crate::domain::{Gallery, Photo};
use crate::template::TemplateRenderer;
use actix_identity::Identity;
use actix_web::{
    HttpResponse,
    web::{Data, Path},
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct GalleriesIndexContext<'a> {
    title: &'a str,
    header: &'a str,
    sub_header: &'a str,
    logged_in: bool,
    galleries: Vec<Gallery>,
}

#[derive(Serialize)]
struct GalleryPageContext<'a> {
    title: &'a str,
    header: &'a str,
    sub_header: &'a str,
    logged_in: bool,
    band: &'a str,
    photo_count: i64,
    photos: Vec<Photo>,
}

/// GET /g/{slug} — public read-only gallery for a single band.
pub async fn get_public_gallery(
    slug: Path<String>,
    templates: Data<dyn TemplateRenderer>,
    database: Data<Arc<dyn DatabaseBackend>>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    let logged_in = is_logged_in(&identity);
    let slug = slug.into_inner();
    let galleries = database.list_galleries().await?;

    let gallery = match galleries.into_iter().find(|g| g.slug == slug) {
        Some(g) => g,
        None => {
            let error_ctx = serde_json::json!({
                "title": "Not Found",
                "header": "Concert Photo Galleries",
                "sub_header": "crusty-metallian.net",
                "logged_in": logged_in,
                "status_code": "404",
                "message": format!("No gallery found for slug: {slug}"),
            });
            let html = templates.render("error.html", &error_ctx)?;
            return Ok(HttpResponse::NotFound()
                .content_type("text/html; charset=utf-8")
                .body(html));
        }
    };

    let photos = database.list_photos_by_band(&gallery.band).await?;

    let context = GalleryPageContext {
        title: &gallery.band,
        header: &gallery.band,
        sub_header: "Concert Photo Galleries",
        logged_in,
        band: &gallery.band,
        photo_count: gallery.photo_count,
        photos,
    };

    let json_context = serde_json::to_value(&context)?;
    let html = templates.render("public_gallery.html", &json_context)?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}

/// GET /galleries — public list of all concert photo galleries.
pub async fn get_galleries_index(
    templates: Data<dyn TemplateRenderer>,
    database: Data<Arc<dyn DatabaseBackend>>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    let logged_in = is_logged_in(&identity);
    let galleries = database.list_galleries().await?;

    let context = GalleriesIndexContext {
        title: "Galleries",
        header: "Galleries",
        sub_header: "Concert Photo Galleries",
        logged_in,
        galleries,
    };

    let json_context = serde_json::to_value(&context)?;
    let html = templates.render("galleries_index.html", &json_context)?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}
