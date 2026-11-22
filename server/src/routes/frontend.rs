// src/routes/frontend.rs

use crate::auth::is_logged_in;
use crate::template::TemplateRenderer;
use actix_identity::Identity;
use actix_web::{HttpResponse, web::Data};
use serde::Serialize;

// module declarations
pub mod delete;
pub mod edit;
pub mod galleries;
pub mod gallery;
pub mod index;
pub mod stats;
pub mod upload;

// re-exports
pub use delete::*;
pub use edit::*;
pub use galleries::*;
pub use gallery::*;
pub use index::*;
pub use stats::*;
pub use upload::*;

#[derive(Serialize)]
struct NotFoundContext {
    title: &'static str,
    header: &'static str,
    sub_header: &'static str,
    status_code: &'static str,
    message: &'static str,
    logged_in: bool,
}

pub async fn not_found(
    templates: Data<dyn TemplateRenderer>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    let not_found_context = NotFoundContext {
        title: "Not Found",
        header: "crusty-metallian.net",
        sub_header: "API",
        status_code: "404",
        message: "The page you are looking for does not exist.",
        logged_in: is_logged_in(&identity),
    };
    let json_ctx = serde_json::to_value(&not_found_context)?;
    let html = templates.render("error.html", &json_ctx)?;
    Ok(actix_web::HttpResponse::NotFound()
        .content_type("text/html; charset=utf-8")
        .body(html))
}
