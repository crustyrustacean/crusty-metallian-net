// src/routes/frontend/delete.rs

use crate::auth::require_login;
use crate::database::DatabaseBackend;
use crate::routes::sse::datastar_sse;
use crate::services::PhotoOperations;
use crate::storage::StorageBackend;
use crate::utils::e400;
use actix_identity::Identity;
use actix_web::Responder;
use actix_web::web::{Data, Path};
use datastar::prelude::PatchElements;
use std::sync::Arc;
use uuid::Uuid;

/// DELETE /gallery/{id} — browser-facing delete.
pub async fn delete_photo(
    path: Path<String>,
    database: Data<Arc<dyn DatabaseBackend>>,
    storage: Data<Arc<dyn StorageBackend>>,
    operations: Data<PhotoOperations>,
    identity: Option<Identity>,
) -> Result<impl Responder, actix_web::Error> {
    require_login(&identity)?;
    let id = Uuid::parse_str(&path.into_inner()).map_err(e400)?;

    operations
        .delete_photo(id, database.as_ref().as_ref(), storage.as_ref().as_ref())
        .await?;

    Ok(datastar_sse([
        PatchElements::new_remove(format!("#photo-{id}")).into_datastar_event(),
    ]))
}
