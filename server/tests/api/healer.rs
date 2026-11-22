// tests/api/healer.rs

use crate::metadata_stub::MetadataStubs;
use server::configuration::DatabaseSettings;
use server::database::{DatabaseBackend, SqliteRepository};
use server::domain::NewRotationEntry;
use server::healer::heal_once;
use server::metadata::MetadataClient;

async fn store() -> (tempfile::TempDir, SqliteRepository) {
    let dir = tempfile::tempdir().expect("temp dir");
    let settings = DatabaseSettings {
        path: dir.path().join("test.db").display().to_string(),
        max_connections: Some(1),
    };
    let repo = SqliteRepository::new(&settings)
        .await
        .expect("store connects and migrates");
    (dir, repo)
}

fn bare(artist: &str, album: &str) -> NewRotationEntry {
    NewRotationEntry {
        listened_date: jiff::civil::date(2026, 10, 7),
        artist: artist.to_string(),
        album: album.to_string(),
        cover: None,
        year: None,
        note: None,
    }
}

#[tokio::test]
async fn a_hit_fills_the_entry_and_clears_it_from_the_candidates() {
    let stubs = MetadataStubs::full_hit("9ca804b8-afbe-3599-b581-e3a3238961a0").await;
    let (_dir, repo) = store().await;
    let metadata = MetadataClient::with_base_urls(stubs.musicbrainz.uri(), stubs.cover_art.uri());

    repo.insert(bare("Sabaton", "Attero Dominatus"))
        .await
        .expect("insert");

    let report = heal_once(&repo, &metadata, 5, 3).await.expect("heal pass");
    assert_eq!(report.healed, 1);
    assert_eq!(report.missed, 0);

    let listed = repo.list().await.expect("list");
    assert!(listed[0].cover.is_some());
    assert_eq!(listed[0].year, Some(2006));
    assert!(repo.list_incomplete(5, 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_total_miss_increments_attempts() {
    let stubs = MetadataStubs::miss().await;
    let (_dir, repo) = store().await;
    let metadata = MetadataClient::with_base_urls(stubs.musicbrainz.uri(), stubs.cover_art.uri());

    repo.insert(bare("Celtic Frost", "To Mega Therion"))
        .await
        .expect("insert");

    let report = heal_once(&repo, &metadata, 5, 3).await.expect("heal pass");
    assert_eq!(report.missed, 1);
    assert_eq!(report.healed, 0);

    assert!(repo.list_incomplete(1, 10).await.unwrap().is_empty());
    assert_eq!(repo.list_incomplete(2, 10).await.unwrap().len(), 1);
}
