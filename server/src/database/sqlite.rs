// src/database/sqlite.rs

use crate::configuration::DatabaseSettings;
use crate::database::{DatabaseBackend, DatabaseError, PhotoStatus};
use crate::domain::{
    ArtistStat, Exif, Gallery, NewRotationEntry, Photo, RotationEntry, UpdatePhoto,
};
use anyhow::Context;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{FromRow, SqlSafeStr, SqlitePool};
use std::str::FromStr;
use uuid::Uuid;
use uuid::fmt::Hyphenated;

fn id_as_text(id: Uuid) -> Hyphenated {
    id.hyphenated()
}

#[derive(Debug, FromRow)]
struct RotationEntryRow {
    id: i64,
    listened_date: String,
    artist: String,
    album: String,
    cover: Option<String>,
    year: Option<i32>,
    note: Option<String>,
}

impl TryFrom<RotationEntryRow> for RotationEntry {
    type Error = anyhow::Error;

    fn try_from(row: RotationEntryRow) -> Result<Self, Self::Error> {
        let id = u64::try_from(row.id).context("rotation entry id overflowed u64")?;
        let listened_date = row
            .listened_date
            .parse::<jiff::civil::Date>()
            .with_context(|| {
                format!(
                    "failed to parse listened_date {:?} for rotation entry {id}",
                    row.listened_date
                )
            })?;
        Ok(RotationEntry {
            id,
            listened_date,
            artist: row.artist,
            album: row.album,
            cover: row.cover,
            year: row.year,
            note: row.note,
        })
    }
}

#[derive(Debug, FromRow)]
struct ArtistStatRow {
    artist: String,
    spins: i64,
}

impl From<ArtistStatRow> for ArtistStat {
    fn from(row: ArtistStatRow) -> Self {
        ArtistStat {
            artist: row.artist,
            spins: row.spins.max(0) as u64,
        }
    }
}

#[derive(Debug, FromRow)]
struct PhotoRow {
    id: Hyphenated,
    band: String,
    tour: String,
    venue: String,
    date_time_original: Option<String>,
    make: Option<String>,
    model: Option<String>,
    lens_make: Option<String>,
    lens_model: Option<String>,
}

impl From<PhotoRow> for Photo {
    fn from(row: PhotoRow) -> Self {
        Photo {
            id: row.id.into_uuid(),
            band: row.band,
            tour: row.tour,
            venue: row.venue,
            exif_data: Exif {
                date_time_original: row.date_time_original,
                make: row.make,
                model: row.model,
                lens_make: row.lens_make,
                lens_model: row.lens_model,
            },
        }
    }
}

#[derive(Debug, FromRow)]
struct GalleryRow {
    band: String,
    photo_count: i64,
    cover_photo_id: Option<String>,
}

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

fn crlf_checksum(migration: &sqlx::migrate::Migration) -> Vec<u8> {
    let lf_sql = migration.sql.as_str().replace("\r\n", "\n");
    let crlf_sql = lf_sql.replace('\n', "\r\n");
    sqlx::migrate::Migration::new(
        migration.version,
        migration.description.clone(),
        migration.migration_type,
        sqlx::AssertSqlSafe(crlf_sql).into_sql_str(),
        migration.no_tx,
    )
    .checksum
    .into_owned()
}

async fn normalize_crlf_migration_checksums(
    pool: &SqlitePool,
    migrator: &sqlx::migrate::Migrator,
) -> Result<(), sqlx::Error> {
    let has_table: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;
    if has_table.is_none() {
        return Ok(());
    }

    for migration in migrator
        .iter()
        .filter(|m| !m.migration_type.is_down_migration())
    {
        let crlf = crlf_checksum(migration);
        let result = sqlx::query(
            "UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2 AND checksum = ?3",
        )
        .bind(migration.checksum.as_ref())
        .bind(migration.version)
        .bind(&crlf)
        .execute(pool)
        .await?;
        if result.rows_affected() > 0 {
            tracing::info!(
                version = migration.version,
                "normalized CRLF migration checksum to LF"
            );
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct SqliteRepository {
    pool: SqlitePool,
}

impl SqliteRepository {
    pub async fn new(db_configuration: &DatabaseSettings) -> Result<Self, anyhow::Error> {
        let db_path = format!("sqlite:{}", db_configuration.path);
        let mut pool_opts = SqlitePoolOptions::new();
        if let Some(max) = db_configuration.max_connections {
            pool_opts = pool_opts.max_connections(max);
        }
        let options = SqliteConnectOptions::from_str(&db_path)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = pool_opts.connect_with(options).await?;

        normalize_crlf_migration_checksums(&pool, &MIGRATOR).await?;
        MIGRATOR.run(&pool).await?;

        Ok(Self { pool })
    }

    async fn insert_photo(&self, photo: Photo, status: PhotoStatus) -> Result<Uuid, DatabaseError> {
        sqlx::query(
            "INSERT INTO photos (id, band, tour, venue, date_time_original, make, model, lens_make, lens_model, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_as_text(photo.id))
        .bind(photo.band)
        .bind(photo.tour)
        .bind(photo.venue)
        .bind(photo.exif_data.date_time_original)
        .bind(photo.exif_data.make)
        .bind(photo.exif_data.model)
        .bind(photo.exif_data.lens_make)
        .bind(photo.exif_data.lens_model)
        .bind(status.as_str())
        .execute(&self.pool)
        .await
        .context("Failed to create the photo.")?;
        Ok(photo.id)
    }
}

#[async_trait]
impl DatabaseBackend for SqliteRepository {
    // ---- Rotation entries ----

    async fn insert(&self, entry: NewRotationEntry) -> Result<RotationEntry, DatabaseError> {
        let result = sqlx::query(
            "INSERT INTO rotation_entries (listened_date, artist, album, cover, year, note) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(entry.listened_date.to_string())
        .bind(&entry.artist)
        .bind(&entry.album)
        .bind(&entry.cover)
        .bind(entry.year)
        .bind(&entry.note)
        .execute(&self.pool)
        .await
        .context("failed to insert rotation entry")?;

        let id = u64::try_from(result.last_insert_rowid())
            .context("inserted rotation entry id overflowed u64")?;

        Ok(RotationEntry {
            id,
            listened_date: entry.listened_date,
            artist: entry.artist,
            album: entry.album,
            cover: entry.cover,
            year: entry.year,
            note: entry.note,
        })
    }

    async fn list(&self) -> Result<Vec<RotationEntry>, DatabaseError> {
        let rows = sqlx::query_as::<_, RotationEntryRow>(
            "SELECT id, listened_date, artist, album, cover, year, note \
             FROM rotation_entries ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list rotation entries")?;

        rows.into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Operation)
    }

    async fn random(&self) -> Result<RotationEntry, DatabaseError> {
        let row = sqlx::query_as::<_, RotationEntryRow>(
            "SELECT id, listened_date, artist, album, cover, year, note \
             FROM rotation_entries ORDER BY RANDOM() LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .context("failed to draw a random rotation entry")?
        .ok_or(DatabaseError::NotFound)?;

        row.try_into().map_err(DatabaseError::Operation)
    }

    async fn artist_stats(&self, limit: i64) -> Result<Vec<ArtistStat>, DatabaseError> {
        let rows = sqlx::query_as::<_, ArtistStatRow>(
            "SELECT artist, COUNT(*) as spins
            FROM rotation_entries
            GROUP BY artist
            ORDER BY spins DESC, artist ASC
            LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to obtain artist stats.")?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_incomplete(
        &self,
        max_attempts: u32,
        limit: i64,
    ) -> Result<Vec<RotationEntry>, DatabaseError> {
        let rows = sqlx::query_as::<_, RotationEntryRow>(
            "SELECT id, listened_date, artist, album, cover, year, note \
             FROM rotation_entries \
             WHERE (cover IS NULL OR year IS NULL) AND heal_attempts < ?1 \
             ORDER BY id LIMIT ?2",
        )
        .bind(max_attempts)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to list incomplete rotation entries")?;

        rows.into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::Operation)
    }

    async fn update_metadata(
        &self,
        id: u64,
        cover: Option<String>,
        year: Option<i32>,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE rotation_entries \
             SET cover = ?1, year = ?2, heal_attempts = 0 \
             WHERE id = ?3",
        )
        .bind(cover)
        .bind(year)
        .bind(i64::try_from(id).context("rotation entry id overflowed i64")?)
        .execute(&self.pool)
        .await
        .context("failed to update rotation entry metadata")?;

        Ok(())
    }

    async fn record_heal_failure(&self, id: u64) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE rotation_entries \
             SET heal_attempts = heal_attempts + 1 \
             WHERE id = ?1",
        )
        .bind(i64::try_from(id).context("rotation entry id overflowed i64")?)
        .execute(&self.pool)
        .await
        .context("failed to record heal failure")?;

        Ok(())
    }

    // ---- Photos & Galleries ----

    async fn create_photo(&self, photo: Photo) -> Result<Uuid, DatabaseError> {
        self.insert_photo(photo, PhotoStatus::Ready).await
    }

    async fn create_uploading_photo(&self, photo: Photo) -> Result<Uuid, DatabaseError> {
        self.insert_photo(photo, PhotoStatus::Uploading).await
    }

    async fn photo_status(&self, id: Uuid) -> Result<Option<PhotoStatus>, DatabaseError> {
        let status: Option<String> = sqlx::query_scalar("SELECT status FROM photos WHERE id = ?")
            .bind(id_as_text(id))
            .fetch_optional(&self.pool)
            .await
            .context("Failed to read photo status.")?;
        status.map(|s| PhotoStatus::parse(&s)).transpose()
    }

    async fn transition_photo_status(
        &self,
        id: Uuid,
        from: PhotoStatus,
        to: PhotoStatus,
    ) -> Result<bool, DatabaseError> {
        if !matches!(
            (from, to),
            (
                PhotoStatus::Uploading,
                PhotoStatus::Ready | PhotoStatus::Deleting
            ) | (PhotoStatus::Ready, PhotoStatus::Deleting)
        ) {
            return Err(DatabaseError::Operation(anyhow::anyhow!(
                "invalid photo status transition: {from:?} to {to:?}"
            )));
        }
        let result = sqlx::query("UPDATE photos SET status = ? WHERE id = ? AND status = ?")
            .bind(to.as_str())
            .bind(id_as_text(id))
            .bind(from.as_str())
            .execute(&self.pool)
            .await
            .context("Failed to transition photo status.")?;
        Ok(result.rows_affected() == 1)
    }

    async fn list_incomplete_photos(&self) -> Result<Vec<(Uuid, PhotoStatus)>, DatabaseError> {
        let rows: Vec<(Hyphenated, String)> =
            sqlx::query_as("SELECT id, status FROM photos WHERE status != 'ready'")
                .fetch_all(&self.pool)
                .await
                .context("Failed to list incomplete photos.")?;
        rows.into_iter()
            .map(|(id, status)| Ok((id.into_uuid(), PhotoStatus::parse(&status)?)))
            .collect()
    }

    async fn delete_incomplete_photo(
        &self,
        id: Uuid,
        status: PhotoStatus,
    ) -> Result<bool, DatabaseError> {
        if status != PhotoStatus::Deleting {
            return Err(DatabaseError::Operation(anyhow::anyhow!(
                "only deleting photos may be removed during cleanup"
            )));
        }
        let result = sqlx::query("DELETE FROM photos WHERE id = ? AND status = ?")
            .bind(id_as_text(id))
            .bind(status.as_str())
            .execute(&self.pool)
            .await
            .context("Failed to remove incomplete photo.")?;
        Ok(result.rows_affected() == 1)
    }

    async fn read_photo(&self, id: Uuid) -> Result<Photo, DatabaseError> {
        let row: Option<PhotoRow> =
            sqlx::query_as("SELECT * FROM photos WHERE id = ? AND status = 'ready'")
                .bind(id_as_text(id))
                .fetch_optional(&self.pool)
                .await
                .context("Failed to fetch the photo.")?;
        let row = row.ok_or(DatabaseError::NotFound)?;
        Ok(row.into())
    }

    async fn list_photos(&self) -> Result<Vec<Photo>, DatabaseError> {
        let rows: Vec<PhotoRow> =
            sqlx::query_as::<_, PhotoRow>("SELECT * FROM photos WHERE status = 'ready'")
                .fetch_all(&self.pool)
                .await
                .context("Failed to list the photos.")?;
        Ok(rows.into_iter().map(Photo::from).collect())
    }

    async fn update_photo(
        &self,
        id: Uuid,
        updated_photo: UpdatePhoto,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "UPDATE photos SET band = ?, tour = ?, venue = ? WHERE id = ? AND status = 'ready'",
        )
        .bind(updated_photo.band)
        .bind(updated_photo.tour)
        .bind(updated_photo.venue)
        .bind(id_as_text(id))
        .execute(&self.pool)
        .await
        .context("Failed to update the photo.")?;
        Ok(())
    }

    async fn delete_photo(&self, id: Uuid) -> Result<(), DatabaseError> {
        sqlx::query("DELETE FROM photos WHERE id = ?")
            .bind(id_as_text(id))
            .execute(&self.pool)
            .await
            .context("Failed to delete the photo.")?;
        Ok(())
    }

    async fn list_galleries(&self) -> Result<Vec<Gallery>, DatabaseError> {
        let rows: Vec<GalleryRow> = sqlx::query_as::<_, GalleryRow>(
            "SELECT band, COUNT(*) as photo_count, (SELECT id FROM photos p2 WHERE p2.band = photos.band AND p2.status = 'ready' ORDER BY rowid DESC LIMIT 1) as cover_photo_id
             FROM photos WHERE status = 'ready' GROUP BY band ORDER BY band ASC",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list galleries.")?;

        Ok(rows
            .into_iter()
            .map(|r| Gallery {
                slug: crate::domain::slugify(&r.band),
                band: r.band,
                photo_count: r.photo_count,
                cover_photo_id: r.cover_photo_id.and_then(|s| Uuid::parse_str(&s).ok()),
            })
            .collect())
    }

    async fn list_photos_by_band(&self, band: &str) -> Result<Vec<Photo>, DatabaseError> {
        let rows: Vec<PhotoRow> = sqlx::query_as::<_, PhotoRow>(
            "SELECT * FROM photos WHERE band = ? AND status = 'ready' ORDER BY rowid ASC",
        )
        .bind(band)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list photos by band.")?;
        Ok(rows.into_iter().map(Photo::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn normalizes_crlf_migration_checksums_on_startup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let cfg = DatabaseSettings {
            path: db_path.to_string_lossy().into_owned(),
            max_connections: Some(2),
        };

        let repo = SqliteRepository::new(&cfg).await.expect("initial setup");
        for migration in MIGRATOR
            .iter()
            .filter(|m| !m.migration_type.is_down_migration())
        {
            let crlf = crlf_checksum(migration);
            sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2")
                .bind(&crlf)
                .bind(migration.version)
                .execute(&repo.pool)
                .await
                .expect("overwrite with CRLF checksum");
        }
        drop(repo);

        let reopened = SqliteRepository::new(&cfg)
            .await
            .expect("should normalize CRLF checksums and succeed");

        for migration in MIGRATOR
            .iter()
            .filter(|m| !m.migration_type.is_down_migration())
        {
            let stored: Vec<u8> =
                sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = ?1")
                    .bind(migration.version)
                    .fetch_one(&reopened.pool)
                    .await
                    .expect("fetch checksum");
            assert_eq!(stored, migration.checksum.as_ref());
        }
    }

    #[tokio::test]
    async fn rejects_genuinely_modified_migration_checksum() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let cfg = DatabaseSettings {
            path: db_path.to_string_lossy().into_owned(),
            max_connections: Some(2),
        };

        let repo = SqliteRepository::new(&cfg).await.expect("initial setup");
        let first_version = MIGRATOR
            .iter()
            .find(|m| !m.migration_type.is_down_migration())
            .expect("at least one migration")
            .version;
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2")
            .bind(vec![0u8; 48])
            .bind(first_version)
            .execute(&repo.pool)
            .await
            .expect("corrupt checksum");
        drop(repo);

        let result = SqliteRepository::new(&cfg).await;
        assert!(
            result.is_err(),
            "genuinely modified migration checksum must still be rejected"
        );
    }
}
