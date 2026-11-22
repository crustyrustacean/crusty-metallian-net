// src/routes/frontend/index.rs

use crate::auth::require_login;
use crate::template::TemplateRenderer;
use actix_identity::Identity;
use actix_web::{HttpResponse, web::Data};
use serde::Serialize;

#[derive(Serialize)]
struct IndexContext<'a> {
    title: &'a str,
    header: &'a str,
    sub_header: &'a str,
    logged_in: bool,
}

pub async fn get_index_page(
    templates: Data<dyn TemplateRenderer>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    require_login(&identity)?;
    let index_context = IndexContext {
        title: "Home",
        header: "Admin Control Panel",
        sub_header: "crusty-metallian.net API",
        logged_in: true,
    };

    let json_index_context = serde_json::to_value(&index_context)?;
    let html = templates.render("index.html", &json_index_context)?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html))
}
