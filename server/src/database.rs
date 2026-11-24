// src/database.rs

use crate::domain::{ArtistStat, Gallery, NewRotationEntry, Photo, RotationEntry, UpdatePhoto};
use crate::utils::error_chain_fmt;
use actix_web::{ResponseError, http::StatusCode};
use async_trait::async_trait;
use uuid::Uuid;

pub mod sqlite;

pub use sqlite::*;

#[derive(thiserror::Error)]
pub enum DatabaseError {
    #[error("record not found")]
    NotFound,
    #[error(transparent)]
    Operation(#[from] anyhow::Error),
}

impl std::fmt::Debug for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for DatabaseError {
    fn status_code(&self) -> StatusCode {
        match self {
            DatabaseError::NotFound => StatusCode::NOT_FOUND,
            DatabaseError::Operation(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Internal photo lifecycle state. Only ready photos are visible to normal queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhotoStatus {
    Uploading,
    Ready,
    Deleting,
}

impl PhotoStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Uploading => "uploading",
            Self::Ready => "ready",
            Self::Deleting => "deleting",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, DatabaseError> {
        match value {
            "uploading" => Ok(Self::Uploading),
            "ready" => Ok(Self::Ready),
            "deleting" => Ok(Self::Deleting),
            _ => Err(DatabaseError::Operation(anyhow::anyhow!(
                "invalid photo status: {value}"
            ))),
        }
    }
}

#[async_trait]
pub trait DatabaseBackend: Send + Sync {
    // ---- Rotation entries ----
    async fn insert(&self, entry: NewRotationEntry) -> Result<RotationEntry, DatabaseError>;
    async fn list(&self) -> Result<Vec<RotationEntry>, DatabaseError>;
    async fn random(&self) -> Result<RotationEntry, DatabaseError>;
    async fn artist_stats(&self, limit: i64) -> Result<Vec<ArtistStat>, DatabaseError>;
    async fn list_incomplete(
        &self,
        max_attempts: u32,
        limit: i64,
    ) -> Result<Vec<RotationEntry>, DatabaseError>;
    async fn update_metadata(
        &self,
        id: u64,
        cover: Option<String>,
        year: Option<i32>,
    ) -> Result<(), DatabaseError>;
    async fn record_heal_failure(&self, id: u64) -> Result<(), DatabaseError>;

    // ---- Photos & Galleries ----
    async fn create_photo(&self, photo: Photo) -> Result<Uuid, DatabaseError>;
    async fn create_uploading_photo(&self, photo: Photo) -> Result<Uuid, DatabaseError>;
    async fn photo_status(&self, id: Uuid) -> Result<Option<PhotoStatus>, DatabaseError>;
    async fn transition_photo_status(
        &self,
        id: Uuid,
        from: PhotoStatus,
        to: PhotoStatus,
    ) -> Result<bool, DatabaseError>;
    async fn list_incomplete_photos(&self) -> Result<Vec<(Uuid, PhotoStatus)>, DatabaseError>;
    async fn delete_incomplete_photo(
        &self,
        id: Uuid,
        status: PhotoStatus,
    ) -> Result<bool, DatabaseError>;
    async fn read_photo(&self, id: Uuid) -> Result<Photo, DatabaseError>;
    async fn list_photos(&self) -> Result<Vec<Photo>, DatabaseError>;
    async fn update_photo(&self, id: Uuid, updated_photo: UpdatePhoto)
    -> Result<(), DatabaseError>;
    async fn delete_photo(&self, id: Uuid) -> Result<(), DatabaseError>;
    async fn list_galleries(&self) -> Result<Vec<Gallery>, DatabaseError>;
    async fn list_photos_by_band(&self, band: &str) -> Result<Vec<Photo>, DatabaseError>;
}
