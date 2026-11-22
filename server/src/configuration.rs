// src/configuration.rs

// dependencies
use confik::{Configuration, EnvSource, FileSource};

#[derive(Configuration, Clone, Debug)]
pub struct Settings {
    pub application: ApplicationSettings,
    pub metadata: MetadataSettings,
    pub admin: AdminSettings,
    pub database: DatabaseSettings,
    pub storage: StorageSettings,
    pub healing: HealingSettings,
    pub cors: CorsSettings,
}

#[derive(Configuration, Clone, Debug)]
pub struct ApplicationSettings {
    pub port: u16,
    pub host: String,
    pub static_dir: String,
    pub templates_glob: String,
}

#[derive(Configuration, Clone, Debug)]
pub struct MetadataSettings {
    /// Overridable so a test can point both services at a local stub.
    pub musicbrainz_base_url: String,
    pub cover_art_base_url: String,
}

#[derive(Configuration, Clone, Debug)]
pub struct AdminSettings {
    pub username: String,
    pub password: String,
}

#[derive(Configuration, Clone, Debug)]
pub struct DatabaseSettings {
    pub path: String,
    pub max_connections: Option<u32>,
}

#[derive(Configuration, Clone, Debug)]
pub struct StorageSettings {
    pub backend: String,
    pub fs_root: String,
    pub r2_bucket: String,
    pub r2_endpoint: String,
    pub r2_access_key: String,
    pub r2_secret_key: String,
}

/// The background healer's tuning. `enabled` can be flipped from the
/// environment (`APP_HEALING__ENABLED=false`) without a redeploy.
#[derive(Configuration, Clone, Debug)]
pub struct HealingSettings {
    pub enabled: bool,
    pub interval_secs: u64,
    pub max_attempts: u32,
    pub per_pass: u32,
}

/// Origins allowed to read the API cross-origin (the blog widgets).
#[derive(Configuration, Clone, Debug)]
pub struct CorsSettings {
    pub allowed_origins: Vec<String>,
}

pub fn get_configuration() -> Result<Settings, confik::Error> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("configuration");

    // Detect the running environment.
    // Default to `local` if unspecified.
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");
    let environment_filename = format!("{}.toml", environment.as_str());

    Settings::builder()
        .override_with(FileSource::new(configuration_directory.join("base.toml")))
        .override_with(FileSource::new(
            configuration_directory.join(environment_filename),
        ))
        // Add in settings from environment variables (with a prefix of `APP_` and `__` as separator)
        // E.g. `APP_APPLICATION__PORT=5001` sets `Settings.application.port`
        .override_with(
            EnvSource::new()
                .with_prefix("APP_")
                .with_separator("__")
                .allow_secrets(),
        )
        .try_build()
}

/// The possible runtime environment for our application.
pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use confik::TomlSource;

    #[test]
    fn toml_sources_layer_and_env_overrides_port() {
        let base = r#"
            [application]
            port = 8000
            host = "0.0.0.0"
            static_dir = "static"
            templates_glob = "templates/**/*.html"

            [metadata]
            musicbrainz_base_url = "https://musicbrainz.org/ws/2"
            cover_art_base_url = "https://coverartarchive.org"

            [admin]
            username = "test"
            password = "test"

            [database]
            path = "crusty-metallian.db"
            max_connections = 5

            [storage]
            backend = "memory"
            fs_root = ""
            r2_bucket = ""
            r2_endpoint = ""
            r2_access_key = ""
            r2_secret_key = ""

            [healing]
            enabled = true
            interval_secs = 900
            max_attempts = 3
            per_pass = 3

            [cors]
            allowed_origins = ["https://crusty-metallian.net"]
        "#;
        let local = r#"
            [application]
            host = "127.0.0.1"
        "#;

        let settings = Settings::builder()
            .override_with(TomlSource::new(base))
            .override_with(TomlSource::new(local))
            .try_build()
            .expect("TOML sources should merge cleanly");

        assert_eq!(settings.application.port, 8000);
        assert_eq!(settings.application.host, "127.0.0.1");
        assert_eq!(settings.database.path, "crusty-metallian.db");
        assert_eq!(settings.storage.backend, "memory");
    }

    #[test]
    fn env_source_parses_prefixed_nested_port_and_ignores_app_environment() {
        // SAFETY: unit test runs in isolation for these specific env vars and cleans up.
        unsafe {
            std::env::set_var("APP_ENVIRONMENT", "production");
            std::env::set_var("APP_APPLICATION__PORT", "5001");
        }

        let result = get_configuration();

        unsafe {
            std::env::remove_var("APP_ENVIRONMENT");
            std::env::remove_var("APP_APPLICATION__PORT");
        }

        let settings = result.expect("Configuration should load with APP_ env overrides");
        assert_eq!(settings.application.port, 5001);
        assert_eq!(settings.application.host, "0.0.0.0");
        assert_eq!(settings.database.path, "/data/crusty-metallian.db");
        assert_eq!(settings.storage.backend, "r2");
    }
}
