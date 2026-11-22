// src/startup.rs

// dependencies
use crate::auth;
use crate::configuration::{AdminSettings, CorsSettings, Settings};
use crate::database::DatabaseBackend;
use crate::metadata::MetadataClient;
use crate::routes::{api, frontend};
use crate::services::PhotoOperations;
use crate::storage::StorageBackend;
use crate::telemetry::request_id;
use crate::template::TemplateRenderer;
use crate::utils::shutdown_signal;
use actix_cors::Cors;
use actix_files::Files;
use actix_identity::IdentityMiddleware;
use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::cookie::Key;
use actix_web::dev::Server;
use actix_web::http::header::{ACCEPT, CONTENT_TYPE, HeaderName, HeaderValue};
use actix_web::{App, HttpResponse, HttpServer, middleware, web, web::Data};
use std::net::TcpListener;
use std::sync::Arc;
use tracing_actix_web::TracingLogger;

const DATASTAR_REQUEST: HeaderName = HeaderName::from_static("datastar-request");

pub struct Application {
    port: u16,
    server: Server,
    database: Arc<dyn DatabaseBackend>,
    storage: Arc<dyn StorageBackend>,
    operations: Data<PhotoOperations>,
}

impl Application {
    pub async fn build(
        configuration: Settings,
        template_renderer: Arc<dyn TemplateRenderer>,
        database: Arc<dyn DatabaseBackend>,
        storage: Arc<dyn StorageBackend>,
        metadata: MetadataClient,
    ) -> Result<Self, anyhow::Error> {
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr()?.port();

        // Derive a 64-byte session signing key from the admin password.
        let seed = if configuration.admin.password.is_empty() {
            b"crusty-metallian-dev-session-key".as_slice()
        } else {
            configuration.admin.password.as_bytes()
        };
        let key_bytes: Vec<u8> = seed.iter().cycle().take(64).copied().collect();
        let session_key = Key::from(&key_bytes);

        let operations = Data::new(PhotoOperations::default());
        if let Err(error) = operations
            .recover_incomplete(database.as_ref(), storage.as_ref())
            .await
        {
            tracing::warn!(error = ?error, "startup photo recovery failed; will retry");
        }

        let server = run(
            listener,
            template_renderer,
            database.clone(),
            storage.clone(),
            operations.clone(),
            metadata,
            configuration.admin,
            session_key,
            configuration.application.static_dir,
            configuration.cors,
        )
        .await?;

        Ok(Self {
            port,
            server,
            database,
            storage,
            operations,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        let Self {
            server,
            database,
            storage,
            operations,
            ..
        } = self;
        let handle = server.handle();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        interval.tick().await; // Startup already performed the first pass.

        tokio::pin!(server);
        loop {
            tokio::select! {
                result = &mut server => {
                    tracing::info!("shutdown complete");
                    return result;
                }
                _ = interval.tick() => {
                    if let Err(error) = operations
                        .recover_incomplete(database.as_ref(), storage.as_ref())
                        .await
                    {
                        tracing::warn!(error = ?error, "photo recovery failed; will retry");
                    }
                }
                _ = shutdown_signal() => {
                    handle.stop(true).await;
                    tracing::info!("shutdown complete");
                    return Ok(());
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run(
    listener: TcpListener,
    template_renderer: Arc<dyn TemplateRenderer>,
    database: Arc<dyn DatabaseBackend>,
    storage: Arc<dyn StorageBackend>,
    operations: Data<PhotoOperations>,
    metadata: MetadataClient,
    admin: AdminSettings,
    session_key: Key,
    static_dir: String,
    cors_settings: CorsSettings,
) -> Result<Server, anyhow::Error> {
    let template_renderer = Data::from(template_renderer);
    let database = Data::new(database);
    let storage = Data::new(storage);
    let metadata = Data::new(metadata);
    let admin = Data::new(admin);
    let cors_origins = cors_settings
        .allowed_origins
        .iter()
        .map(|origin| {
            HeaderValue::from_str(origin)
                .map_err(|_| anyhow::anyhow!("invalid CORS origin in configuration: {origin:?}"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let server = HttpServer::new(move || {
        let cors_origins = cors_origins.clone();
        let cors = Cors::default()
            .allowed_methods(["GET", "HEAD"])
            .allowed_headers([CONTENT_TYPE, ACCEPT, DATASTAR_REQUEST])
            .allowed_origin_fn(move |origin, _request_head| {
                cors_origins.iter().any(|allowed| allowed == origin)
            });

        App::new()
            .wrap(middleware::from_fn(request_id))
            .wrap(TracingLogger::default())
            .wrap(IdentityMiddleware::default())
            .wrap(SessionMiddleware::new(
                CookieSessionStore::default(),
                session_key.clone(),
            ))
            // CORS must wrap the application so preflight requests receive
            // headers before route handlers run.
            .wrap(cors)
            // --- public HTML routes ---
            .route("/galleries", web::get().to(frontend::get_galleries_index))
            .route("/g/{slug}", web::get().to(frontend::get_public_gallery))
            // --- auth routes ---
            .route("/login", web::get().to(auth::get_login_page))
            .route("/login", web::post().to(auth::post_login))
            .route("/logout", web::get().to(auth::get_logout))
            // --- admin HTML routes (session auth checked in handler) ---
            .route("/", web::get().to(frontend::get_index_page))
            .route("/stats", web::get().to(frontend::get_stats_page))
            .route("/gallery", web::get().to(frontend::get_gallery_page))
            .route("/upload", web::get().to(frontend::get_upload_page))
            .route("/upload", web::post().to(frontend::post_upload))
            .route("/gallery/{id}", web::delete().to(frontend::delete_photo))
            .route("/gallery/{id}/edit", web::get().to(frontend::edit_photo))
            .route("/gallery/{id}", web::put().to(frontend::update_photo))
            .route("/gallery/{id}/cancel", web::get().to(frontend::cancel_edit))
            // --- versioned API routes ---
            .service(
                web::scope("/api/v1")
                    .route("/health_check", web::get().to(api::health_check))
                    .service(
                        web::resource("/rotation")
                            .route(web::get().to(api::get_rotation_entry))
                            .route(web::post().to(api::post_rotation_entry_ds)),
                    )
                    .route("/stats", web::get().to(api::get_artist_stats))
                    .route("/galleries", web::get().to(api::list_galleries))
                    .route("/photos", web::post().to(api::create_photo))
                    .route("/photos/{id}", web::get().to(api::read_photo))
                    .route("/photos/{id}/image", web::get().to(api::get_photo_image))
                    .route("/photos/{id}", web::put().to(api::update_photo))
                    .route("/photos/{id}", web::delete().to(api::delete_photo)),
            )
            .service(
                Files::new("/static", static_dir.clone())
                    .prefer_utf8(true)
                    .default_handler(web::to(HttpResponse::NotFound)),
            )
            .default_service(web::to(frontend::not_found))
            .app_data(template_renderer.clone())
            .app_data(database.clone())
            .app_data(storage.clone())
            .app_data(operations.clone())
            .app_data(metadata.clone())
            .app_data(admin.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
