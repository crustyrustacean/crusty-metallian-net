// tests/api/database.rs

use server::configuration::DatabaseSettings;
use server::database::{DatabaseBackend, DatabaseError, SqliteRepository};
use server::domain::NewRotationEntry;

fn entry(artist: &str, album: &str) -> NewRotationEntry {
    NewRotationEntry {
        listened_date: jiff::civil::date(2026, 10, 7),
        artist: artist.to_string(),
        album: album.to_string(),
        cover: None,
        year: None,
        note: None,
    }
}

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

#[tokio::test]
async fn insert_assigns_monotonically_increasing_ids() {
    let (_dir, repo) = store().await;

    let first = repo
        .insert(entry("Judas Priest", "Invincible Shield"))
        .await
        .expect("insert");
    let second = repo
        .insert(entry("Judas Priest", "Painkiller"))
        .await
        .expect("insert");

    assert_eq!(first.id, 1);
    assert_eq!(second.id, first.id + 1);
}

#[tokio::test]
async fn insert_round_trips_optional_fields() {
    let (_dir, repo) = store().await;

    let full = repo
        .insert(NewRotationEntry {
            listened_date: jiff::civil::date(2026, 10, 7),
            artist: "Judas Priest".to_string(),
            album: "Invincible Shield".to_string(),
            cover: Some("https://coverart.example/shield.jpg".to_string()),
            year: Some(2024),
            note: Some("Classic".to_string()),
        })
        .await
        .expect("insert");

    assert_eq!(
        full.cover.as_deref(),
        Some("https://coverart.example/shield.jpg")
    );
    assert_eq!(full.year, Some(2024));
    assert_eq!(full.note.as_deref(), Some("Classic"));
}

#[tokio::test]
async fn random_on_an_empty_store_is_not_found() {
    let (_dir, repo) = store().await;

    assert!(matches!(repo.random().await, Err(DatabaseError::NotFound)));
}

#[tokio::test]
async fn entries_survive_closing_and_reopening_the_database() {
    let (dir, repo) = store().await;

    repo.insert(entry("Judas Priest", "Invincible Shield"))
        .await
        .expect("insert");
    drop(repo);

    let settings = DatabaseSettings {
        path: dir.path().join("test.db").display().to_string(),
        max_connections: Some(1),
    };
    let reopened = SqliteRepository::new(&settings).await.expect("reopen");
    let listed = reopened.list().await.expect("list after reopen");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].album, "Invincible Shield");
}
