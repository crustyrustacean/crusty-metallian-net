// src/routes/api/photos.rs

use crate::auth::require_api_login;
use crate::database::DatabaseBackend;
use crate::domain::UpdatePhoto;
use crate::services::PhotoOperations;
use crate::storage::StorageBackend;
use crate::utils::{e400, e500};
use actix_identity::Identity;
use actix_multipart::form::{MultipartForm, tempfile::TempFile, text::Text};
use actix_web::HttpResponse;
use actix_web::http::StatusCode;
use actix_web::web::{self, Data, Form, Path};
use std::fs;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, MultipartForm)]
pub struct CreatePhotoForm {
    band: Text<String>,
    tour: Text<String>,
    venue: Text<String>,
    #[multipart(rename = "file")]
    photo_file: Vec<TempFile>,
}

/// POST /api/v1/photos
pub async fn create_photo(
    MultipartForm(form): MultipartForm<CreatePhotoForm>,
    database: Data<Arc<dyn DatabaseBackend>>,
    storage: Data<Arc<dyn StorageBackend>>,
    operations: Data<PhotoOperations>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    require_api_login(&identity)?;
    let photo_file = form
        .photo_file
        .into_iter()
        .next()
        .ok_or_else(|| e400("a file upload is required"))?;
    let photo_file_bytes = web::block(move || fs::read(photo_file.file.path()))
        .await
        .map_err(e500)?
        .map_err(e400)?
        .into();

    let id = operations
        .create_photo_from_bytes(
            photo_file_bytes,
            form.band.into_inner(),
            form.tour.into_inner(),
            form.venue.into_inner(),
            database.as_ref().as_ref(),
            storage.as_ref().as_ref(),
        )
        .await?;

    Ok(HttpResponse::Ok().json(id))
}

/// GET /api/v1/photos/{id}
pub async fn read_photo(
    path: Path<String>,
    database: Data<Arc<dyn DatabaseBackend>>,
) -> Result<HttpResponse, actix_web::Error> {
    let id = Uuid::parse_str(&path.into_inner()).map_err(e400)?;
    let photo = database.read_photo(id).await?;
    Ok(HttpResponse::Ok().json(photo))
}

/// GET /api/v1/photos/{id}/image
pub async fn get_photo_image(
    path: Path<String>,
    database: Data<Arc<dyn DatabaseBackend>>,
    storage: Data<Arc<dyn StorageBackend>>,
) -> Result<HttpResponse, actix_web::Error> {
    let id = Uuid::parse_str(&path.into_inner()).map_err(e400)?;
    database.read_photo(id).await?;
    let bytes = storage.find(id).await?;
    Ok(HttpResponse::Ok().content_type("image/jpeg").body(bytes))
}

/// PUT /api/v1/photos/{id}
pub async fn update_photo(
    path: Path<String>,
    form: Form<UpdatePhoto>,
    database: Data<Arc<dyn DatabaseBackend>>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    require_api_login(&identity)?;
    let id = Uuid::parse_str(&path.into_inner()).map_err(e400)?;
    database.update_photo(id, form.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

/// DELETE /api/v1/photos/{id}
pub async fn delete_photo(
    path: Path<String>,
    database: Data<Arc<dyn DatabaseBackend>>,
    storage: Data<Arc<dyn StorageBackend>>,
    operations: Data<PhotoOperations>,
    identity: Option<Identity>,
) -> Result<HttpResponse, actix_web::Error> {
    require_api_login(&identity)?;
    let id = Uuid::parse_str(&path.into_inner()).map_err(e400)?;
    operations
        .delete_photo(id, database.as_ref().as_ref(), storage.as_ref().as_ref())
        .await?;
    Ok(HttpResponse::Ok().status(StatusCode::NO_CONTENT).finish())
}
