// src/bin/main.rs

// dependencies
use server::configuration::get_configuration;
use server::database::{DatabaseBackend, SqliteRepository};
use server::healer::spawn_healer;
use server::metadata::MetadataClient;
use server::startup::Application;
use server::telemetry::{get_subscriber, init_subscriber};
use server::template::{TemplateRenderer, tera::TeraRenderer};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = get_subscriber("server".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration()?;
    let template_renderer: Arc<dyn TemplateRenderer> =
        Arc::new(TeraRenderer::new(&configuration.application.templates_glob)?);
    let database: Arc<dyn DatabaseBackend> =
        Arc::new(SqliteRepository::new(&configuration.database).await?);
    let metadata = MetadataClient::with_base_urls(
        configuration.metadata.musicbrainz_base_url.clone(),
        configuration.metadata.cover_art_base_url.clone(),
    );

    spawn_healer(
        database.clone(),
        metadata.clone(),
        configuration.healing.clone(),
    );

    let application =
        Application::build(configuration, template_renderer, database, metadata).await?;
    application.run_until_stopped().await?;

    Ok(())
}
