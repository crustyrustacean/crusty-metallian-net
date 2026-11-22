// src/healer.rs

use crate::database::{DatabaseBackend, DatabaseError};
use crate::metadata::{AlbumQuery, MetadataClient};
use std::sync::Arc;
use std::time::Duration;

/// What one healing pass accomplished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealReport {
    pub healed: u32,
    pub missed: u32,
}

/// One healing pass over up to `per_pass` candidates.
pub async fn heal_once(
    database: &dyn DatabaseBackend,
    metadata: &MetadataClient,
    max_attempts: u32,
    per_pass: usize,
) -> Result<HealReport, DatabaseError> {
    let candidates = database
        .list_incomplete(max_attempts, per_pass as i64)
        .await?;
    let mut report = HealReport {
        healed: 0,
        missed: 0,
    };

    for entry in candidates {
        let query = AlbumQuery::new(&entry.artist, &entry.album);
        let found = metadata.lookup_best_effort(&query).await;

        if found.cover.is_some() || found.year.is_some() {
            database
                .update_metadata(entry.id, found.cover, found.year)
                .await?;
            report.healed += 1;
            tracing::info!(
                entry = entry.id,
                artist = %entry.artist,
                album = %entry.album,
                "healed rotation entry"
            );
        } else {
            database.record_heal_failure(entry.id).await?;
            report.missed += 1;
            tracing::warn!(
                entry = entry.id,
                artist = %entry.artist,
                album = %entry.album,
                attempts = "incremented",
                "heal lookup found nothing"
            );
        }
    }

    Ok(report)
}

/// Start the healer alongside the server: one pass immediately, then a pass
/// every `interval_secs`.
pub fn spawn_healer(
    database: Arc<dyn DatabaseBackend>,
    metadata: MetadataClient,
    settings: crate::configuration::HealingSettings,
) {
    if !settings.enabled {
        tracing::info!("metadata healer disabled by configuration");
        return;
    }

    tokio::spawn(async move {
        loop {
            match heal_once(
                database.as_ref(),
                &metadata,
                settings.max_attempts,
                settings.per_pass as usize,
            )
            .await
            {
                Ok(report) if report.healed + report.missed > 0 => {
                    tracing::info!(
                        healed = report.healed,
                        missed = report.missed,
                        "heal pass complete"
                    );
                }
                Ok(_) => {}
                Err(error) => tracing::error!(error = ?error, "heal pass failed"),
            }

            tokio::time::sleep(Duration::from_secs(settings.interval_secs)).await;
        }
    });
}
