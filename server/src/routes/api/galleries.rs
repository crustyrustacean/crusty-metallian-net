// src/routes/api/galleries.rs

use crate::database::DatabaseBackend;
use actix_web::{HttpResponse, web::Data};
use std::sync::Arc;

/// GET /api/v1/galleries
pub async fn list_galleries(
    database: Data<Arc<dyn DatabaseBackend>>,
) -> Result<HttpResponse, actix_web::Error> {
    let galleries = database.list_galleries().await?;
    Ok(HttpResponse::Ok().json(galleries))
}
