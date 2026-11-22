// src/domain.rs

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---- Rotation / MusicFeed domain types ----

#[derive(Clone, Debug, Serialize)]
pub struct RotationEntry {
    pub id: u64,
    pub listened_date: Date,
    pub artist: String,
    pub album: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewRotationEntry {
    pub listened_date: Date,
    pub artist: String,
    pub album: String,
    pub cover: Option<String>,
    pub year: Option<i32>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtistStat {
    pub artist: String,
    pub spins: u64,
}

// ---- Photos / Galleries domain types ----

#[derive(Debug, Serialize)]
pub struct Photo {
    pub id: Uuid,
    pub band: String,
    pub tour: String,
    pub venue: String,
    pub exif_data: Exif,
}

#[derive(Debug, Default, Serialize)]
pub struct Exif {
    pub date_time_original: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens_make: Option<String>,
    pub lens_model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePhoto {
    pub band: String,
    pub tour: String,
    pub venue: String,
}

/// A gallery is derived from the `band` field on photos — no separate table.
/// One gallery per unique band name.
#[derive(Debug, Serialize)]
pub struct Gallery {
    pub band: String,
    pub slug: String,
    pub photo_count: i64,
    pub cover_photo_id: Option<Uuid>,
}

/// Convert a band name into a URL-friendly slug.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '-' || c == '_' {
                '-'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        .trim_matches('-')
        .replace("--", "-")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Iron Maiden"), "iron-maiden");
    }

    #[test]
    fn slugify_handles_multiple_spaces() {
        assert_eq!(slugify("Black  Sabbath"), "black-sabbath");
    }

    #[test]
    fn slugify_drops_non_ascii() {
        assert_eq!(slugify("Mot\u{f6}rhead"), "motrhead");
    }
}
